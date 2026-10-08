use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use reqwest::Client;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use tokio::sync::watch;

use crate::{protocol::ServiceError, storage::AppStorage};

const CATALOG_URL: &str = "https://models.dev/api.json";
const CACHE_AGE_MS: i64 = 6 * 60 * 60 * 1000;
const MAX_CATALOG_BYTES: usize = 16 * 1024 * 1024;

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

pub(super) struct PricingCatalog {
    state: &'static str,
    fetched_at_unix_ms: Option<i64>,
    models: Vec<ModelRates>,
}

impl PricingCatalog {
    pub(super) fn metadata(&self) -> Value {
        serde_json::json!({
            "source": "models.dev",
            "state": self.state,
            "fetchedAtUnixMs": self.fetched_at_unix_ms,
        })
    }

    pub(super) fn install_temporary_prices(&self, database: &Connection) -> rusqlite::Result<()> {
        database.execute_batch(
            "CREATE TEMP TABLE models_dev_prices (
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
    Ok(())
}

async fn fetch_catalog() -> Result<String, ()> {
    let client = Client::builder()
        .user_agent(concat!("OpenChat/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| ())?;
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
