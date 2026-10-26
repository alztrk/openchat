//! Loads and caches models.dev pricing metadata for provider model catalogs.
//!
//! Catalog parsing keeps only validated price and context fields. Network
//! failures produce an explicit stale or unavailable catalog state rather than
//! fabricated prices.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures_util::StreamExt;
use reqwest::Client;
use rusqlite::{OptionalExtension, params};
use serde_json::Value;
use tokio::sync::watch;

use crate::{
    protocol::ServiceError,
    storage::{AppStorage, StorageConnection},
};

const CATALOG_URL: &str = "https://models.dev/api.json";
const CACHE_AGE_MS: i64 = 6 * 60 * 60 * 1000;
const MAX_CATALOG_BYTES: usize = 16 * 1024 * 1024;

struct ModelImageSupportCache {
    database_path: PathBuf,
    fetched_at_unix_ms: i64,
    models_by_provider: HashMap<String, HashMap<String, bool>>,
}

static MODEL_IMAGE_SUPPORT_CACHE: Mutex<Option<ModelImageSupportCache>> = Mutex::new(None);

#[derive(Clone, Copy)]
struct TokenRates {
    input: f64,
    output: f64,
    reasoning: Option<f64>,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
}

struct ModelRates {
    provider_id: String,
    model_id: String,
    tiers: Vec<(i64, TokenRates)>,
}

/// Validated models.dev prices together with the catalog freshness state.
pub(super) struct PricingCatalog {
    state: &'static str,
    fetched_at_unix_ms: Option<i64>,
    models: Vec<ModelRates>,
}

impl PricingCatalog {
    /// Returns the source and freshness metadata exposed to statistics callers.
    pub(super) fn metadata(&self) -> Value {
        serde_json::json!({
            "source": "models.dev",
            "state": self.state,
            "fetchedAtUnixMs": self.fetched_at_unix_ms,
        })
    }

    /// Installs validated price tiers in connection-local tables for one query.
    pub(super) fn install_temporary_prices(
        &self,
        database: &mut StorageConnection<'_>,
    ) -> rusqlite::Result<()> {
        // Temporary tables survive on the service's reused SQLite connection.
        database.execute_batch(
            "DROP TABLE IF EXISTS temp.models_dev_event_costs;
            DROP TABLE IF EXISTS temp.models_dev_prices;
            CREATE TEMP TABLE models_dev_prices (
                provider_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                context_size INTEGER NOT NULL,
                input_usd_per_million REAL NOT NULL,
                output_usd_per_million REAL NOT NULL,
                reasoning_usd_per_million REAL,
                cache_read_usd_per_million REAL,
                cache_write_usd_per_million REAL,
                PRIMARY KEY (provider_id, model_id, context_size)
            );
            CREATE TEMP TABLE models_dev_event_costs (
                event_id TEXT PRIMARY KEY NOT NULL,
                cost_usd REAL NOT NULL CHECK (cost_usd >= 0)
            );",
        )?;

        let transaction = database.unchecked_transaction()?;
        {
            let mut statement = transaction.prepare_cached(
                "INSERT INTO models_dev_prices (
                    provider_id, model_id, context_size, input_usd_per_million,
                    output_usd_per_million, reasoning_usd_per_million,
                    cache_read_usd_per_million, cache_write_usd_per_million
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for model in &self.models {
                for (context_size, rates) in &model.tiers {
                    statement.execute(params![
                        model.provider_id,
                        model.model_id,
                        context_size,
                        rates.input,
                        rates.output,
                        rates.reasoning,
                        rates.cache_read,
                        rates.cache_write,
                    ])?;
                }
            }
        }
        transaction.commit()?;

        database.execute_batch(
            "INSERT INTO models_dev_event_costs (event_id, cost_usd)
             SELECT event.event_id,
                (
                    (
                        (event.input_tokens
                            - COALESCE(event.cached_input_tokens, 0)
                            - COALESCE(event.cache_write_tokens, 0))
                            * price.input_usd_per_million
                        + COALESCE(event.cached_input_tokens, 0)
                            * COALESCE(price.cache_read_usd_per_million,
                                price.input_usd_per_million)
                        + COALESCE(event.cache_write_tokens, 0)
                            * COALESCE(price.cache_write_usd_per_million,
                                price.input_usd_per_million)
                        + (event.output_tokens - COALESCE(event.reasoning_tokens, 0))
                            * price.output_usd_per_million
                        + COALESCE(event.reasoning_tokens, 0)
                            * COALESCE(price.reasoning_usd_per_million,
                                price.output_usd_per_million)
                    ) / 1000000.0
                )
             FROM usage_events AS event
             JOIN models_dev_prices AS price
               ON price.provider_id = CASE event.provider_id
                    WHEN 'chatgpt_api' THEN 'openai'
                    WHEN 'gemini' THEN 'google'
                    ELSE event.provider_id
                  END
              AND price.model_id = event.model_id
              AND price.context_size = (
                    SELECT MAX(tier.context_size)
                    FROM models_dev_prices AS tier
                    WHERE tier.provider_id = price.provider_id
                      AND tier.model_id = price.model_id
                      AND tier.context_size <= event.input_tokens
              )
             WHERE event.event_kind = 'request'
               AND event.input_tokens IS NOT NULL
               AND event.output_tokens IS NOT NULL
               AND event.fast_requested = 0
               AND COALESCE(lower(event.service_tier), '') IN ('', 'default', 'standard')
               AND event.input_tokens >=
                    COALESCE(event.cached_input_tokens, 0)
                    + COALESCE(event.cache_write_tokens, 0)
               AND event.output_tokens >= COALESCE(event.reasoning_tokens, 0)
               AND (
                    price.reasoning_usd_per_million IS NULL
                    OR price.reasoning_usd_per_million = price.output_usd_per_million
                    OR event.reasoning_tokens IS NOT NULL
               );",
        )?;
        Ok(())
    }
}

/// Loads fresh cached pricing or fetches and validates the remote catalog.
///
/// A stale valid cache is returned when refresh fails. With no valid cache, the
/// result has an explicit unavailable state and no model prices.
pub(super) async fn load(
    storage: &AppStorage,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<PricingCatalog, ServiceError> {
    let cached = load_cached_catalog(storage)?;
    let now = current_time_millis().map_err(|_| storage_error())?;
    if let Some((fetched_at, catalog_json)) = &cached
        && now >= *fetched_at
        && now - *fetched_at < CACHE_AGE_MS
        && let Some(models) = parse_catalog(catalog_json)
    {
        return Ok(PricingCatalog {
            state: "current",
            fetched_at_unix_ms: Some(*fetched_at),
            models,
        });
    }

    let fetched = tokio::select! {
        changed = cancellation.changed() => {
            let _ = changed;
            return Err(ServiceError::new(
                "request_cancelled",
                "The statistics request was cancelled.",
                true,
            ));
        }
        result = fetch_catalog() => result,
    };

    if let Ok(catalog_json) = fetched
        && let Some(models) = parse_catalog(&catalog_json)
    {
        let fetched_at = current_time_millis().map_err(|_| storage_error())?;
        save_catalog(storage, fetched_at, &catalog_json)?;
        return Ok(PricingCatalog {
            state: "current",
            fetched_at_unix_ms: Some(fetched_at),
            models,
        });
    }

    if let Some((fetched_at, catalog_json)) = cached
        && let Some(models) = parse_catalog(&catalog_json)
    {
        return Ok(PricingCatalog {
            state: "stale",
            fetched_at_unix_ms: Some(fetched_at),
            models,
        });
    }

    Ok(PricingCatalog {
        state: "unavailable",
        fetched_at_unix_ms: None,
        models: Vec::new(),
    })
}

/// Returns freshness metadata and one model's validated catalog details.
pub(super) async fn model_details(
    storage: &AppStorage,
    provider_id: &str,
    model_id: &str,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    let catalog = load(storage, cancellation).await?;
    let cached = load_cached_catalog(storage)?;
    let model = cached
        .and_then(|(_, json)| serde_json::from_str::<Value>(&json).ok())
        .and_then(|catalog| model_from_catalog(&catalog, provider_id, model_id));

    Ok(serde_json::json!({
        "state": catalog.state,
        "fetchedAtUnixMs": catalog.fetched_at_unix_ms,
        "model": model,
    }))
}

/// Returns freshness metadata and validated details for requested models.
pub(super) async fn model_details_batch(
    storage: &AppStorage,
    requested_models: &[(String, String)],
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    let catalog = load(storage, cancellation).await?;
    let cached = load_cached_catalog(storage)?;
    let cached_catalog = cached.and_then(|(_, json)| serde_json::from_str::<Value>(&json).ok());
    let models = requested_models
        .iter()
        .map(|(provider_id, model_id)| {
            serde_json::json!({
                "providerId": provider_id,
                "modelId": model_id,
                "model": cached_catalog
                    .as_ref()
                    .and_then(|catalog| model_from_catalog(catalog, provider_id, model_id)),
            })
        })
        .collect::<Vec<_>>();

    Ok(serde_json::json!({
        "state": catalog.state,
        "fetchedAtUnixMs": catalog.fetched_at_unix_ms,
        "models": models,
    }))
}

fn model_from_catalog(catalog: &Value, provider_id: &str, model_id: &str) -> Option<Value> {
    let catalog_provider_id = catalog_provider_id(provider_id);
    let provider = catalog.get(catalog_provider_id)?;
    let model = provider.get("models")?.get(model_id)?;
    let context_window = positive_limit(model, "context");
    let input_token_limit = positive_limit(model, "input");
    let maximum_output = positive_limit(model, "output");
    let input_modalities = string_list(
        model
            .get("modalities")
            .and_then(|modalities| modalities.get("input")),
    );
    let output_modalities = string_list(
        model
            .get("modalities")
            .and_then(|modalities| modalities.get("output")),
    );
    let supports_images = model
        .pointer("/modalities/input")
        .and_then(Value::as_array)
        .map(|_| input_modalities.contains(&"image"));
    let cost = model.get("cost").filter(|value| value.is_object());
    let pricing = cost.and_then(|cost| {
        let tiers = parse_model_tiers(cost)?;
        let base_rates = tiers.first().map(|(_, rates)| *rates)?;
        Some(serde_json::json!({
            "input": base_rates.input,
            "output": base_rates.output,
            "reasoning": base_rates.reasoning,
            "cacheRead": base_rates.cache_read,
            "cacheWrite": base_rates.cache_write,
            "tiers": tiers.into_iter().skip(1).map(|(context, rates)| {
                serde_json::json!({
                    "context": context,
                    "input": rates.input,
                    "output": rates.output,
                    "reasoning": rates.reasoning,
                    "cacheRead": rates.cache_read,
                    "cacheWrite": rates.cache_write,
                })
            }).collect::<Vec<_>>(),
        }))
    });

    Some(serde_json::json!({
        "providerId": catalog_provider_id,
        "providerName": provider.get("name").and_then(Value::as_str),
        "modelId": model_id,
        "modelName": model.get("name").and_then(Value::as_str),
        "contextWindow": context_window,
        "inputTokenLimit": input_token_limit,
        "maximumOutput": maximum_output,
        "inputModalities": input_modalities,
        "outputModalities": output_modalities,
        "supportsImages": supports_images,
        "supportsTools": model.get("tool_call").and_then(Value::as_bool),
        "supportsReasoning": model.get("reasoning").and_then(Value::as_bool),
        "pricing": pricing,
    }))
}

pub(super) fn cached_model_supports_images(
    storage: &AppStorage,
    provider_id: &str,
    model_id: &str,
) -> Option<bool> {
    let model_ids = [model_id.to_owned()];
    let mut support = cached_models_support_images(storage, provider_id, &model_ids);
    support.remove(model_id)
}

pub(super) fn cached_models_support_images(
    storage: &AppStorage,
    provider_id: &str,
    model_ids: &[String],
) -> HashMap<String, bool> {
    if model_ids.is_empty() {
        return HashMap::new();
    }
    let Ok(database) = storage.connect() else {
        return HashMap::new();
    };

    let fetched_at_unix_ms = database
        .query_row(
            "SELECT fetched_at_unix_ms
             FROM models_dev_catalog_cache WHERE cache_id = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .ok()
        .flatten();
    let Some(fetched_at_unix_ms) = fetched_at_unix_ms else {
        return HashMap::new();
    };
    let database_path = storage.database_path().to_path_buf();
    let mut cache = match MODEL_IMAGE_SUPPORT_CACHE.lock() {
        Ok(cache) => cache,
        Err(poisoned) => poisoned.into_inner(),
    };

    let catalog_provider_id = catalog_provider_id(provider_id);
    let cache_matches_catalog = cache.as_ref().is_some_and(|snapshot| {
        snapshot.database_path == database_path && snapshot.fetched_at_unix_ms == fetched_at_unix_ms
    });
    if !cache_matches_catalog {
        *cache = Some(ModelImageSupportCache {
            database_path,
            fetched_at_unix_ms,
            models_by_provider: HashMap::new(),
        });
    }

    if let Some(snapshot) = cache.as_ref()
        && snapshot
            .models_by_provider
            .contains_key(catalog_provider_id)
    {
        return model_image_support_for_ids(snapshot, provider_id, model_ids);
    }

    let Ok(rows) = database.query_map(
        "SELECT model.key,
                EXISTS (
                    SELECT 1
                    FROM json_each(model.value, '$.modalities.input') AS modality
                    WHERE modality.value = 'image'
                ) AS supports_images
         FROM models_dev_catalog_cache AS cache
         CROSS JOIN json_each(cache.catalog_json) AS provider
         CROSS JOIN json_each(provider.value, '$.models') AS model
         WHERE cache.cache_id = 1 AND provider.key = ?1",
        [catalog_provider_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?)),
    ) else {
        return HashMap::new();
    };
    let Ok(rows) = rows.collect::<rusqlite::Result<Vec<_>>>() else {
        return HashMap::new();
    };
    let provider_models: HashMap<_, _> = rows.into_iter().collect();

    let Some(snapshot) = cache.as_mut() else {
        return HashMap::new();
    };
    snapshot
        .models_by_provider
        .insert(catalog_provider_id.to_owned(), provider_models);
    model_image_support_for_ids(snapshot, provider_id, model_ids)
}

fn model_image_support_for_ids(
    cache: &ModelImageSupportCache,
    provider_id: &str,
    model_ids: &[String],
) -> HashMap<String, bool> {
    let Some(provider_models) = cache
        .models_by_provider
        .get(catalog_provider_id(provider_id))
    else {
        return HashMap::new();
    };

    model_ids
        .iter()
        .filter_map(|model_id| {
            provider_models
                .get(model_id)
                .map(|supports_images| (model_id.clone(), *supports_images))
        })
        .collect()
}

fn catalog_provider_id(provider_id: &str) -> &str {
    match provider_id {
        "chatgpt" | "chatgpt_api" => "openai",
        "gemini" => "google",
        provider => provider,
    }
}

fn positive_limit(model: &Value, field: &str) -> Option<i64> {
    model
        .get("limit")?
        .get(field)?
        .as_i64()
        .filter(|value| *value > 0)
}

fn string_list(value: Option<&Value>) -> Vec<&str> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|value| !value.is_empty())
        .collect()
}

fn load_cached_catalog(storage: &AppStorage) -> Result<Option<(i64, String)>, ServiceError> {
    storage
        .connect()
        .map_err(|_| storage_error())?
        .query_row(
            "SELECT fetched_at_unix_ms, catalog_json
             FROM models_dev_catalog_cache WHERE cache_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| storage_error())
}

fn save_catalog(
    storage: &AppStorage,
    fetched_at_unix_ms: i64,
    catalog_json: &str,
) -> Result<(), ServiceError> {
    storage
        .connect()
        .map_err(|_| storage_error())?
        .execute(
            "INSERT INTO models_dev_catalog_cache (cache_id, fetched_at_unix_ms, catalog_json)
             VALUES (1, ?1, ?2)
             ON CONFLICT(cache_id) DO UPDATE SET
                fetched_at_unix_ms = excluded.fetched_at_unix_ms,
                catalog_json = excluded.catalog_json",
            params![fetched_at_unix_ms, catalog_json],
        )
        .map_err(|_| storage_error())?;
    let mut cache = match MODEL_IMAGE_SUPPORT_CACHE.lock() {
        Ok(cache) => cache,
        Err(poisoned) => poisoned.into_inner(),
    };
    if cache
        .as_ref()
        .is_some_and(|cached| cached.database_path.as_path() == storage.database_path())
    {
        *cache = None;
    }
    Ok(())
}

async fn fetch_catalog() -> Result<String, ()> {
    let client = build_catalog_http_client()?;
    let response = client.get(CATALOG_URL).send().await.map_err(|_| ())?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|length| length > MAX_CATALOG_BYTES as u64)
    {
        return Err(());
    }

    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ())?;
        if body.len().saturating_add(chunk.len()) > MAX_CATALOG_BYTES {
            return Err(());
        }
        body.extend_from_slice(&chunk);
    }
    let catalog = serde_json::from_slice::<Value>(&body).map_err(|_| ())?;
    if !catalog.is_object() || parse_catalog_value(&catalog).is_none() {
        return Err(());
    }
    String::from_utf8(body).map_err(|_| ())
}

/// Builds a bounded client for the fixed public pricing catalog without following redirects.
fn build_catalog_http_client() -> Result<Client, ()> {
    Client::builder()
        .user_agent(concat!("OpenChat/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| ())
}

fn current_time_millis() -> std::io::Result<i64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    i64::try_from(elapsed.as_millis())
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn parse_catalog(catalog_json: &str) -> Option<Vec<ModelRates>> {
    let catalog = serde_json::from_str::<Value>(catalog_json).ok()?;
    parse_catalog_value(&catalog)
}

fn parse_catalog_value(catalog: &Value) -> Option<Vec<ModelRates>> {
    let providers = catalog.as_object()?;
    let mut models = Vec::new();
    for (provider_id, provider) in providers {
        let Some(provider_models) = provider.get("models").and_then(Value::as_object) else {
            continue;
        };
        for (model_id, model) in provider_models {
            let Some(cost) = model.get("cost").filter(|cost| cost.is_object()) else {
                continue;
            };
            let Some(tiers) = parse_model_tiers(cost) else {
                continue;
            };
            models.push(ModelRates {
                provider_id: provider_id.clone(),
                model_id: model_id.clone(),
                tiers,
            });
        }
    }
    (!models.is_empty()).then_some(models)
}

fn parse_model_tiers(cost: &Value) -> Option<Vec<(i64, TokenRates)>> {
    let base = parse_rates(cost, None)?;
    let Some(raw_tiers) = cost.get("tiers") else {
        if cost.get("context_over_200k").is_some() {
            return None;
        }
        return Some(vec![(0, base)]);
    };
    let tiers = raw_tiers.as_array()?;
    let mut parsed = vec![(0, base)];
    for tier in tiers {
        let tier_metadata = tier.get("tier")?;
        if tier_metadata.get("type")?.as_str()? != "context" {
            return None;
        }
        let context_size = tier_metadata.get("size")?.as_i64()?;
        if context_size <= 0 {
            return None;
        }
        parsed.push((context_size, parse_rates(tier, Some(base))?));
    }
    parsed.sort_by_key(|(context_size, _)| *context_size);
    if parsed.windows(2).any(|window| window[0].0 == window[1].0) {
        return None;
    }
    Some(parsed)
}

fn parse_rates(value: &Value, inherited: Option<TokenRates>) -> Option<TokenRates> {
    let input = price(value, "input").or_else(|| inherited.map(|rates| rates.input))?;
    let output = price(value, "output").or_else(|| inherited.map(|rates| rates.output))?;
    Some(TokenRates {
        input,
        output,
        reasoning: price(value, "reasoning")
            .or_else(|| inherited.and_then(|rates| rates.reasoning)),
        cache_read: price(value, "cache_read")
            .or_else(|| inherited.and_then(|rates| rates.cache_read)),
        cache_write: price(value, "cache_write")
            .or_else(|| inherited.and_then(|rates| rates.cache_write)),
    })
}

fn price(value: &Value, key: &str) -> Option<f64> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .filter(|price| price.is_finite() && *price >= 0.0)
}

fn storage_error() -> ServiceError {
    ServiceError::new(
        "usage_statistics_unavailable",
        "Local usage statistics could not be loaded.",
        true,
    )
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        path::PathBuf,
        time::{Duration, Instant},
    };

    use super::{
        build_catalog_http_client, cached_model_supports_images, cached_models_support_images,
        model_from_catalog,
    };
    use crate::storage::AppStorage;
    use rusqlite::params;
    use serde_json::json;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-models-dev-cache-test-{}",
                uuid::Uuid::new_v4().simple()
            ));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test directory");
        }
    }

    /// Confirms a redirected pricing request cannot reach an unrelated loopback host.
    #[tokio::test]
    async fn catalog_http_client_does_not_follow_redirects() {
        let target_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind redirect target fixture");
        let target_address = target_listener
            .local_addr()
            .expect("read redirect target fixture address");
        let target_task = tokio::spawn(async move {
            let accepted =
                tokio::time::timeout(Duration::from_secs(1), target_listener.accept()).await;
            let Ok(Ok((mut stream, _))) = accepted else {
                return false;
            };
            read_request_headers(&mut stream).await;
            stream
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .expect("write redirect target response");
            true
        });

        let source_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind redirect source fixture");
        let source_address = source_listener
            .local_addr()
            .expect("read redirect source fixture address");
        let target_url = format!("http://{target_address}/redirect-target");
        let source_task = tokio::spawn(async move {
            let (mut stream, _) = source_listener
                .accept()
                .await
                .expect("accept redirect source request");
            read_request_headers(&mut stream).await;
            let response = format!(
                "HTTP/1.1 302 Found\r\nLocation: {target_url}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write redirect source response");
        });

        let response = build_catalog_http_client()
            .expect("build pricing catalog client")
            .get(format!("http://{source_address}/api.json"))
            .send()
            .await
            .expect("receive redirect response");
        source_task.await.expect("join redirect source");
        let target_was_contacted = target_task.await.expect("join redirect target");

        assert_eq!(response.status(), reqwest::StatusCode::FOUND);
        assert!(
            !target_was_contacted,
            "the redirect target must not receive the catalog request"
        );
    }

    async fn read_request_headers(stream: &mut tokio::net::TcpStream) {
        let mut request = Vec::new();
        let mut byte = [0; 1];
        while !request.ends_with(b"\r\n\r\n") {
            let read = stream.read(&mut byte).await.expect("read request headers");
            assert_ne!(read, 0, "client closed before sending request headers");
            request.push(byte[0]);
        }
    }

    #[test]
    fn cached_image_support_lookup_returns_only_requested_model_flags() {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");
        let filler = "x".repeat(6 * 1024 * 1024);
        let catalog = json!({
            "openai": {
                "models": {
                    "gpt-image": {
                        "description": filler,
                        "modalities": {"input": ["text", "image"]}
                    },
                    "gpt-text": {"modalities": {"input": ["text"]}}
                }
            }
        })
        .to_string();
        let catalog_bytes = catalog.len();
        storage
            .connect()
            .expect("connect to test storage")
            .execute(
                "INSERT INTO models_dev_catalog_cache (cache_id, fetched_at_unix_ms, catalog_json)
                 VALUES (1, 0, ?1)",
                params![catalog],
            )
            .expect("store test catalog");

        let model_ids = [
            "gpt-image".to_owned(),
            "gpt-text".to_owned(),
            "missing".to_owned(),
        ];
        let started = Instant::now();
        let mut image_support = HashMap::new();
        for _ in 0..5 {
            image_support = cached_models_support_images(&storage, "chatgpt", &model_ids);
        }
        let average_ms = started.elapsed().as_secs_f64() * 1000.0 / 5.0;

        assert_eq!(image_support.get("gpt-image"), Some(&true));
        assert_eq!(image_support.get("gpt-text"), Some(&false));
        assert!(!image_support.contains_key("missing"));
        assert_eq!(
            cached_model_supports_images(&storage, "chatgpt", "gpt-image"),
            Some(true)
        );
        eprintln!("6 MiB model capability lookup average: {average_ms:.2} ms");
        assert!(catalog_bytes >= 6 * 1024 * 1024);
    }

    #[test]
    fn model_details_match_provider_and_model_ids_exactly() {
        let catalog = json!({
            "openai": {
                "name": "OpenAI",
                "models": {
                    "gpt-example": {
                        "name": "GPT Example",
                        "limit": {"context": 128000, "output": 16000},
                        "modalities": {"input": ["text", "image"], "output": ["text"]},
                        "tool_call": true,
                        "cost": {"input": 2.5, "output": 10.0, "reasoning": 12.0}
                    }
                }
            }
        });

        let details = model_from_catalog(&catalog, "chatgpt_api", "gpt-example")
            .expect("exact model should be returned");
        assert_eq!(details["providerName"], "OpenAI");
        assert_eq!(details["contextWindow"], 128000);
        assert_eq!(details["maximumOutput"], 16000);
        assert_eq!(details["pricing"]["input"], 2.5);
        assert_eq!(details["pricing"]["output"], 10.0);
        assert_eq!(details["pricing"]["reasoning"], 12.0);
        assert!(details["supportsTools"].as_bool().unwrap_or(false));
        assert!(model_from_catalog(&catalog, "chatgpt_api", "other").is_none());
    }
}
