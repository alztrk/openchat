//! Read-only quota and balance snapshots for providers whose API keys OpenChat already stores.
//!
//! Requests go only to fixed provider endpoints, authenticated with protected saved keys.
//! Credentials and upstream response bodies never cross the RPC boundary: only validated
//! numeric fields are returned, and per-provider failures become typed snapshot errors.

use std::{sync::OnceLock, time::Duration};

use reqwest::{Client, StatusCode};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::watch;
use zeroize::Zeroizing;

use crate::{
    protocol::ServiceError,
    provider_credentials::ProviderCredentials,
    response_budget::{MAX_PROVIDER_BALANCE_RESPONSE_BYTES, read_bounded_response_body},
    storage::AppStorage,
};

const OPENROUTER_KEY_URL: &str = "https://openrouter.ai/api/v1/key";
const OPENROUTER_CREDITS_URL: &str = "https://openrouter.ai/api/v1/credits";
const DEEPSEEK_BALANCE_URL: &str = "https://api.deepseek.com/user/balance";
const OPENCODE_GO_USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";

/// Providers whose saved API key can be used to read a quota or balance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BalanceProvider {
    OpenRouter,
    DeepSeek,
    OpenCodeGo,
}

impl BalanceProvider {
    /// Returns the saved-key route and the identifier reported to the renderer.
    fn id(self) -> &'static str {
        match self {
            Self::OpenRouter => "openrouter",
            Self::DeepSeek => "deepseek",
            Self::OpenCodeGo => "opencode",
        }
    }
}

/// A failed balance request. `Unsupported` is an endpoint the provider does not offer to this key
/// (HTTP 403 or 404), as opposed to a rejected key or a provider failure.
#[derive(Debug)]
enum FetchError {
    Unsupported(ServiceError),
    Failed(ServiceError),
}

impl From<ServiceError> for FetchError {
    fn from(error: ServiceError) -> Self {
        Self::Failed(error)
    }
}

impl FetchError {
    fn into_error(self) -> ServiceError {
        match self {
            Self::Unsupported(error) | Self::Failed(error) => error,
        }
    }
}

/// Percentage-based usage window such as a key limit or a rolling provider quota.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuotaWindow {
    label: &'static str,
    window_seconds: Option<u64>,
    used_percent: f64,
    reset_at_unix_ms: Option<i64>,
}

impl QuotaWindow {
    /// Rejects percentages that are not finite or are negative.
    fn new(
        label: &'static str,
        window_seconds: Option<u64>,
        used_percent: f64,
        reset_at_unix_ms: Option<i64>,
    ) -> Option<Self> {
        (used_percent.is_finite() && used_percent >= 0.0).then_some(Self {
            label,
            window_seconds,
            used_percent,
            reset_at_unix_ms,
        })
    }
}

/// Money amounts reported for one provider balance, in the currency the provider reported.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct BalanceAmounts {
    label: &'static str,
    currency: String,
    total: Option<f64>,
    granted: Option<f64>,
    topped_up: Option<f64>,
    limit: Option<f64>,
    remaining: Option<f64>,
    used: Option<f64>,
}

/// Validated quota and balance information for one provider, or the reason it is unavailable.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderBalanceSnapshot {
    provider_id: &'static str,
    status: &'static str,
    reason_code: Option<&'static str>,
    error: Option<ServiceError>,
    windows: Vec<QuotaWindow>,
    balances: Vec<BalanceAmounts>,
}

impl ProviderBalanceSnapshot {
    fn available(provider: BalanceProvider) -> Self {
        Self {
            provider_id: provider.id(),
            status: "available",
            reason_code: None,
            error: None,
            windows: Vec::new(),
            balances: Vec::new(),
        }
    }

    fn unavailable(provider: BalanceProvider, error: ServiceError) -> Self {
        Self {
            status: "unavailable",
            reason_code: Some(error.code),
            error: Some(error),
            ..Self::available(provider)
        }
    }

    /// A provider response without any recognised window or balance is not a usable snapshot.
    fn require_quota_data(self) -> Result<Self, ServiceError> {
        if self.windows.is_empty() && self.balances.is_empty() {
            Err(invalid_response_error())
        } else {
            Ok(self)
        }
    }
}

/// Reads balances and quotas for every provider that has a saved key.
///
/// Providers without a usable saved key are omitted, as is OpenCode Go when its usage endpoint
/// is not offered to the key. Credentials and upstream error bodies never cross the RPC boundary.
pub(crate) async fn list(
    storage: &AppStorage,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    if *cancellation.borrow() {
        return Err(cancelled_error());
    }
    let credentials = ProviderCredentials::load(storage)?;
    let openrouter_key = credentials.api_key(BalanceProvider::OpenRouter.id(), None)?;
    let deepseek_key = credentials.api_key(BalanceProvider::DeepSeek.id(), None)?;
    let opencode_key = credentials.api_key(BalanceProvider::OpenCodeGo.id(), None)?;
    let client = client()?;
    if *cancellation.borrow() {
        return Err(cancelled_error());
    }

    let load = async {
        let (openrouter, deepseek, opencode) = tokio::join!(
            load_snapshot(&client, BalanceProvider::OpenRouter, openrouter_key),
            load_snapshot(&client, BalanceProvider::DeepSeek, deepseek_key),
            load_snapshot(&client, BalanceProvider::OpenCodeGo, opencode_key),
        );
        [openrouter, deepseek, opencode]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
    };
    let providers = tokio::select! {
        biased;
        _ = cancellation.changed() => return Err(cancelled_error()),
        providers = load => providers,
    };
    Ok(json!({ "providers": providers }))
}

fn client() -> Result<Client, ServiceError> {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client.clone());
    }
    let client = Client::builder()
        .user_agent(concat!("OpenChat/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| network_error().with_details(error.without_url().to_string()))?;
    Ok(CLIENT.get_or_init(|| client).clone())
}

async fn load_snapshot(
    client: &Client,
    provider: BalanceProvider,
    api_key: Option<Zeroizing<String>>,
) -> Option<ProviderBalanceSnapshot> {
    let api_key = api_key.filter(|key| is_usable_api_key(key.as_str()))?;
    let loaded = match provider {
        BalanceProvider::OpenRouter => load_openrouter(client, api_key.as_str()).await,
        BalanceProvider::DeepSeek => fetch_json(client, DEEPSEEK_BALANCE_URL, api_key.as_str())
            .await
            .and_then(|balance| Ok(parse_deepseek_balance(&balance)?)),
        BalanceProvider::OpenCodeGo => fetch_json(client, OPENCODE_GO_USAGE_URL, api_key.as_str())
            .await
            .and_then(|usage| Ok(parse_opencode_go_usage(&usage)?)),
    };
    match loaded {
        Ok(snapshot) => Some(snapshot),
        // OpenCode Go usage is only available to some console keys; others simply have no card.
        Err(FetchError::Unsupported(_)) if provider == BalanceProvider::OpenCodeGo => None,
        Err(error) => Some(ProviderBalanceSnapshot::unavailable(
            provider,
            error.into_error(),
        )),
    }
}

async fn load_openrouter(
    client: &Client,
    api_key: &str,
) -> Result<ProviderBalanceSnapshot, FetchError> {
    let (key, credits) = tokio::join!(
        fetch_json(client, OPENROUTER_KEY_URL, api_key),
        fetch_json(client, OPENROUTER_CREDITS_URL, api_key),
    );
    let key = key?;
    let credits = optional_endpoint(credits)?;
    Ok(parse_openrouter_balances(&key, credits.as_ref())?)
}

/// Treats a supplementary endpoint the key cannot read or the provider answers badly as absent,
/// keeping every other failure.
fn optional_endpoint(result: Result<Value, FetchError>) -> Result<Option<Value>, FetchError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(FetchError::Unsupported(_)) => Ok(None),
        Err(FetchError::Failed(error))
            if matches!(
                error.code,
                "authentication_required" | "invalid_provider_response"
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn is_usable_api_key(api_key: &str) -> bool {
    !api_key.is_empty()
        && api_key.len() <= 4096
        && api_key.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

async fn fetch_json(client: &Client, url: &str, api_key: &str) -> Result<Value, FetchError> {
    let response = client
        .get(url)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                timeout_error()
            } else {
                network_error().with_details(error.without_url().to_string())
            }
        })?;
    if !response.status().is_success() {
        return Err(http_error(response.status()));
    }
    let body = read_bounded_response_body(response, MAX_PROVIDER_BALANCE_RESPONSE_BYTES).await?;
    serde_json::from_slice(&body).map_err(|_| invalid_response_error().into())
}

/// Accepts finite amounts the renderer can represent exactly and folds `-0.0` into `0.0`.
fn safe_number(number: f64) -> Option<f64> {
    let maximum_safe_integer = 9_007_199_254_740_991_f64;
    if !number.is_finite() || number.abs() > maximum_safe_integer {
        None
    } else if number == 0.0 {
        Some(0.0)
    } else {
        Some(number)
    }
}

fn finite_number(value: &Value) -> Option<f64> {
    safe_number(
        value
            .as_f64()
            .or_else(|| value.as_str()?.trim().parse().ok())?,
    )
}

fn parse_openrouter_balances(
    key: &Value,
    credits: Option<&Value>,
) -> Result<ProviderBalanceSnapshot, ServiceError> {
    let data = key
        .get("data")
        .filter(|data| data.is_object())
        .ok_or_else(invalid_response_error)?;
    let mut snapshot = ProviderBalanceSnapshot::available(BalanceProvider::OpenRouter);
    let limit = finite_number(&data["limit"]);
    // `usage` is all-time spend, while the limit resets: measure the limit against its own window.
    let (window_seconds, used_field) = match data["limit_reset"].as_str() {
        Some("daily") => (Some(86_400), "usage_daily"),
        Some("weekly") => (Some(604_800), "usage_weekly"),
        Some("monthly") => (Some(2_592_000), "usage_monthly"),
        _ => (None, "usage"),
    };
    let used = finite_number(&data[used_field]);
    let remaining = finite_number(&data["limit_remaining"]).or_else(|| safe_number(limit? - used?));
    if limit.is_some() || used.is_some() || remaining.is_some() {
        snapshot.balances.push(BalanceAmounts {
            label: "key",
            currency: "USD".into(),
            limit,
            remaining,
            used,
            ..BalanceAmounts::default()
        });
    }
    if let (Some(limit), Some(remaining)) = (limit.filter(|limit| *limit > 0.0), remaining) {
        snapshot.windows.extend(QuotaWindow::new(
            "key",
            window_seconds,
            (limit - remaining) / limit * 100.0,
            None,
        ));
    }
    for (field, label) in [
        ("usage_daily", "daily"),
        ("usage_weekly", "weekly"),
        ("usage_monthly", "monthly"),
    ] {
        if let Some(used) = finite_number(&data[field]) {
            snapshot.balances.push(BalanceAmounts {
                label,
                currency: "USD".into(),
                used: Some(used),
                ..BalanceAmounts::default()
            });
        }
    }
    // Credits are supplementary: a body without account data keeps the key snapshot.
    if let Some(data) = credits
        .and_then(|credits| credits.get("data"))
        .filter(|data| data.is_object())
    {
        let total = finite_number(&data["total_credits"]);
        let used = finite_number(&data["total_usage"]);
        if let (Some(total), Some(used)) = (total.filter(|total| *total > 0.0), used) {
            snapshot.windows.extend(QuotaWindow::new(
                "credits",
                None,
                used / total * 100.0,
                None,
            ));
        }
        if total.is_some() || used.is_some() {
            snapshot.balances.push(BalanceAmounts {
                label: "credits",
                currency: "USD".into(),
                total,
                used,
                remaining: total
                    .zip(used)
                    .and_then(|(total, used)| safe_number(total - used)),
                ..BalanceAmounts::default()
            });
        }
    }
    snapshot.require_quota_data()
}

fn parse_deepseek_balance(balance: &Value) -> Result<ProviderBalanceSnapshot, ServiceError> {
    let mut snapshot = ProviderBalanceSnapshot::available(BalanceProvider::DeepSeek);
    if balance["is_available"].as_bool() == Some(false) {
        snapshot.status = "unavailable";
        snapshot.reason_code = Some("insufficient_balance");
    }
    let balance_infos = balance["balance_infos"]
        .as_array()
        .ok_or_else(invalid_response_error)?;
    for info in balance_infos {
        let Some(currency) = info["currency"].as_str().filter(|currency| {
            currency.len() == 3 && currency.bytes().all(|b| b.is_ascii_uppercase())
        }) else {
            continue;
        };
        // One balance per currency keeps the renderer's list keys unique.
        if snapshot
            .balances
            .iter()
            .any(|balance| balance.currency == currency)
        {
            continue;
        }
        let total = finite_number(&info["total_balance"]);
        let granted = finite_number(&info["granted_balance"]);
        let topped_up = finite_number(&info["topped_up_balance"]);
        if total.is_some() || granted.is_some() || topped_up.is_some() {
            snapshot.balances.push(BalanceAmounts {
                label: "balance",
                currency: currency.into(),
                total,
                granted,
                topped_up,
                ..BalanceAmounts::default()
            });
        }
    }
    snapshot.require_quota_data()
}

fn parse_opencode_go_usage(usage: &Value) -> Result<ProviderBalanceSnapshot, ServiceError> {
    let mut snapshot = ProviderBalanceSnapshot::available(BalanceProvider::OpenCodeGo);
    let windows = usage
        .get("usage")
        .filter(|windows| windows.is_object())
        .unwrap_or(usage);
    for (label, window_seconds) in [
        ("rolling", Some(18_000)),
        ("weekly", Some(604_800)),
        ("monthly", None),
    ] {
        let window = &windows[label];
        if let Some(percent) = finite_number(&window["percent"]) {
            snapshot.windows.extend(QuotaWindow::new(
                label,
                window_seconds,
                percent,
                reset_unix_ms(&window["resetsAt"]),
            ));
        }
    }
    snapshot.require_quota_data()
}

fn reset_unix_ms(value: &Value) -> Option<i64> {
    let reset = time::OffsetDateTime::parse(
        value.as_str()?,
        &time::format_description::well_known::Rfc3339,
    )
    .ok()?;
    i64::try_from(reset.unix_timestamp_nanos() / 1_000_000).ok()
}

fn http_error(status: StatusCode) -> FetchError {
    let (code, message, retryable) = match status {
        StatusCode::UNAUTHORIZED => (
            "authentication_required",
            "The provider rejected the saved API key. Check the connection in Settings.",
            false,
        ),
        StatusCode::FORBIDDEN => (
            "permission_denied",
            "The saved API key is not allowed to read balance information.",
            false,
        ),
        StatusCode::NOT_FOUND => (
            "provider_request_failed",
            "The provider does not offer balance information for this API key.",
            false,
        ),
        StatusCode::TOO_MANY_REQUESTS => (
            "rate_limited",
            "The provider reported a usage limit. Try again later.",
            true,
        ),
        _ => (
            "provider_request_failed",
            "The provider could not return its balance information.",
            status.is_server_error(),
        ),
    };
    let error = ServiceError::new(code, message, retryable)
        .with_details(format!("HTTP status {}", status.as_u16()));
    if matches!(status, StatusCode::FORBIDDEN | StatusCode::NOT_FOUND) {
        FetchError::Unsupported(error)
    } else {
        FetchError::Failed(error)
    }
}

fn network_error() -> ServiceError {
    ServiceError::new(
        "network_unavailable",
        "The provider could not be reached. Check the network and try again.",
        true,
    )
}

fn timeout_error() -> ServiceError {
    ServiceError::new(
        "provider_timeout",
        "The provider did not return its balance information in time. Try again.",
        true,
    )
}

fn invalid_response_error() -> ServiceError {
    ServiceError::new(
        "invalid_provider_response",
        "The provider returned balance information OpenChat could not read.",
        false,
    )
}

fn cancelled_error() -> ServiceError {
    ServiceError::new(
        "request_cancelled",
        "The provider balance request was cancelled.",
        true,
    )
}

#[cfg(test)]
mod tests {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        task::JoinHandle,
    };

    use super::*;

    /// Serves one fixed HTTP response and returns the request head the client sent.
    async fn serve_once(raw_response: &'static [u8]) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local balance fixture");
        let address = listener.local_addr().expect("read fixture address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept fixture request");
            let mut head = Vec::new();
            let mut byte = [0; 1];
            while !head.ends_with(b"\r\n\r\n") {
                let read = stream.read(&mut byte).await.expect("read request head");
                assert_ne!(read, 0, "client closed before sending request headers");
                head.push(byte[0]);
            }
            stream
                .write_all(raw_response)
                .await
                .expect("write fixture response");
            String::from_utf8(head).expect("request head should be UTF-8")
        });
        (format!("http://{address}/balance"), server)
    }

    #[tokio::test]
    async fn providers_without_a_usable_key_issue_no_requests() {
        let client = client().expect("build balance client");
        for provider in [
            BalanceProvider::OpenRouter,
            BalanceProvider::DeepSeek,
            BalanceProvider::OpenCodeGo,
        ] {
            for api_key in [
                None,
                Some(" "),
                Some("bad\nkey"),
                Some("bad key"),
                Some("k\u{e9}y"),
            ] {
                let api_key = api_key.map(|key| Zeroizing::new(key.to_owned()));
                assert!(load_snapshot(&client, provider, api_key).await.is_none());
            }
        }
    }

    #[tokio::test]
    async fn balance_request_sends_the_saved_key_as_a_bearer_token() {
        let (url, server) = serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"balance\":2.5}",
        )
        .await;

        let value = fetch_json(
            &client().expect("build balance client"),
            &url,
            "fixture-key",
        )
        .await
        .expect("successful balance response");

        assert_eq!(value["balance"], 2.5);
        let head = server.await.expect("fixture server should complete");
        assert!(head.contains("authorization: Bearer fixture-key"));
    }

    #[tokio::test]
    async fn balance_request_does_not_follow_redirects() {
        let (url, server) = serve_once(
            b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )
        .await;

        let error = fetch_json(
            &client().expect("build balance client"),
            &url,
            "fixture-key",
        )
        .await
        .expect_err("redirect responses are not successful")
        .into_error();

        assert_eq!(error.code, "provider_request_failed");
        server.await.expect("fixture server should complete");
    }

    #[tokio::test]
    async fn balance_request_rejects_oversized_responses() {
        let (url, server) =
            serve_once(b"HTTP/1.1 200 OK\r\nContent-Length: 1048576\r\nConnection: close\r\n\r\n")
                .await;

        let error = fetch_json(
            &client().expect("build balance client"),
            &url,
            "fixture-key",
        )
        .await
        .expect_err("responses above the byte limit are rejected")
        .into_error();

        assert_eq!(error.code, "response_too_large");
        server.await.expect("fixture server should complete");
    }

    #[tokio::test]
    async fn balance_request_reports_timeouts() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local balance fixture");
        let address = listener.local_addr().expect("read fixture address");
        let server = tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.expect("accept fixture request");
            tokio::time::sleep(Duration::from_secs(5)).await;
        });
        let client = Client::builder()
            .timeout(Duration::from_millis(100))
            .build()
            .expect("build short-timeout client");

        let error = fetch_json(&client, &format!("http://{address}/balance"), "fixture-key")
            .await
            .expect_err("an unanswered request times out")
            .into_error();

        assert_eq!(error.code, "provider_timeout");
        server.abort();
    }

    #[tokio::test]
    async fn balance_request_rejects_invalid_json() {
        let (url, server) = serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nnot json",
        )
        .await;

        let error = fetch_json(
            &client().expect("build balance client"),
            &url,
            "fixture-key",
        )
        .await
        .expect_err("a body that is not JSON is invalid")
        .into_error();

        assert_eq!(error.code, "invalid_provider_response");
        server.await.expect("fixture server should complete");
    }

    #[test]
    fn openrouter_key_and_credits_become_windows_and_balances() {
        let key = json!({
            "data": {
                "limit": 20,
                "limit_remaining": 15,
                "usage": 5,
                "usage_daily": 1,
                "usage_weekly": 3,
                "usage_monthly": 5,
                "limit_reset": "monthly"
            }
        });
        let credits = json!({"data": {"total_credits": 100, "total_usage": 30}});

        let snapshot = parse_openrouter_balances(&key, Some(&credits))
            .expect("valid OpenRouter key and credits");

        assert_eq!(snapshot.windows[0].used_percent, 25.0);
        assert_eq!(snapshot.windows[0].window_seconds, Some(2_592_000));
        assert_eq!(snapshot.windows[1].used_percent, 30.0);
        assert_eq!(snapshot.balances.len(), 5);
        let credit_balance = snapshot
            .balances
            .last()
            .expect("credits balance should be last");
        assert_eq!(credit_balance.remaining, Some(70.0));
    }

    #[test]
    fn openrouter_without_credits_keeps_the_key_snapshot() {
        let key = json!({"data": {"limit": 20, "limit_remaining": 15, "usage": 5}});

        let snapshot =
            parse_openrouter_balances(&key, None).expect("a key snapshot without credits");

        assert_eq!(snapshot.windows.len(), 1);
        assert_eq!(snapshot.balances.len(), 1);
    }

    #[test]
    fn openrouter_derives_the_remaining_limit_from_usage() {
        let key = json!({"data": {"limit": 10, "usage": 4}});

        let snapshot = parse_openrouter_balances(&key, None).expect("derived remaining limit");

        assert_eq!(snapshot.balances[0].remaining, Some(6.0));
        assert_eq!(snapshot.windows[0].used_percent, 40.0);
    }

    #[test]
    fn openrouter_measures_a_resetting_limit_against_its_own_window() {
        let key = json!({
            "data": {
                "limit": 20,
                "usage": 100,
                "usage_daily": 4,
                "usage_weekly": 9,
                "usage_monthly": 30,
                "limit_reset": "daily"
            }
        });

        let snapshot =
            parse_openrouter_balances(&key, None).expect("daily limit without remaining");

        assert_eq!(snapshot.balances[0].used, Some(4.0));
        assert_eq!(snapshot.balances[0].remaining, Some(16.0));
        assert_eq!(snapshot.windows[0].used_percent, 20.0);
        assert_eq!(snapshot.windows[0].window_seconds, Some(86_400));

        let key = json!({"data": {"limit": 20, "usage": 100, "usage_weekly": 9, "limit_reset": "weekly"}});
        let snapshot = parse_openrouter_balances(&key, None).expect("weekly limit");
        assert_eq!(snapshot.balances[0].remaining, Some(11.0));
        assert_eq!(snapshot.windows[0].window_seconds, Some(604_800));
    }

    #[test]
    fn openrouter_prefers_the_reported_remaining_limit() {
        let key = json!({
            "data": {
                "limit": 20,
                "limit_remaining": 12,
                "usage": 100,
                "usage_monthly": 3,
                "limit_reset": "monthly"
            }
        });

        let snapshot = parse_openrouter_balances(&key, None).expect("reported remaining limit");

        assert_eq!(snapshot.balances[0].used, Some(3.0));
        assert_eq!(snapshot.balances[0].remaining, Some(12.0));
        assert_eq!(snapshot.windows[0].used_percent, 40.0);
    }

    #[test]
    fn openrouter_does_not_derive_a_window_without_its_usage() {
        let key = json!({"data": {"limit": 20, "usage": 100, "limit_reset": "daily"}});

        let snapshot = parse_openrouter_balances(&key, None).expect("limit without window usage");

        assert_eq!(snapshot.balances[0].used, None);
        assert_eq!(snapshot.balances[0].remaining, None);
        assert!(snapshot.windows.is_empty());
    }

    #[test]
    fn openrouter_ignores_non_finite_and_negative_values() {
        let key = json!({
            "data": {"limit": "NaN", "limit_remaining": 5, "usage": "inf", "usage_daily": "NaN"}
        });
        let snapshot = parse_openrouter_balances(&key, None).expect("remaining limit is finite");
        assert!(snapshot.windows.is_empty());
        assert_eq!(snapshot.balances.len(), 1);
        assert_eq!(snapshot.balances[0].used, None);

        let key = json!({"data": {"limit": 10, "limit_remaining": 15}});
        let snapshot = parse_openrouter_balances(&key, None).expect("remaining limit is finite");
        assert!(snapshot.windows.is_empty());
    }

    #[test]
    fn openrouter_rejects_responses_without_usable_data() {
        assert!(parse_openrouter_balances(&json!({"data": {}}), None).is_err());
        assert!(parse_openrouter_balances(&json!({}), None).is_err());
    }

    #[test]
    fn openrouter_credits_without_account_data_keep_the_key_snapshot() {
        let key = json!({"data": {"limit": 20, "limit_remaining": 15, "usage": 5}});

        for credits in [json!({}), json!({"data": null}), json!({"data": []})] {
            let snapshot = parse_openrouter_balances(&key, Some(&credits))
                .expect("the key snapshot survives unusable credits");
            assert_eq!(snapshot.windows.len(), 1);
            assert_eq!(snapshot.balances.len(), 1);
        }
    }

    #[test]
    fn openrouter_credits_access_errors_are_optional_but_other_failures_are_not() {
        for status in [
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
        ] {
            let skipped = optional_endpoint(Err(http_error(status)))
                .expect("access errors skip the credits endpoint");
            assert!(skipped.is_none());
        }

        let error = optional_endpoint(Err(http_error(StatusCode::TOO_MANY_REQUESTS)))
            .expect_err("rate limits are surfaced")
            .into_error();
        assert_eq!(error.code, "rate_limited");
        let error = optional_endpoint(Err(timeout_error().into()))
            .expect_err("timeouts are surfaced")
            .into_error();
        assert_eq!(error.code, "provider_timeout");
        let skipped = optional_endpoint(Err(invalid_response_error().into()))
            .expect("unreadable credits keep the key snapshot");
        assert!(skipped.is_none());
    }

    #[test]
    fn amounts_are_bounded_and_never_negative_zero() {
        let maximum_safe_integer = 9_007_199_254_740_991_f64;
        assert_eq!(
            finite_number(&json!(maximum_safe_integer)),
            Some(maximum_safe_integer)
        );
        assert_eq!(finite_number(&json!(maximum_safe_integer * 2.0)), None);
        assert_eq!(finite_number(&json!("1e300")), None);
        assert_eq!(finite_number(&json!(-1e300)), None);
        for zero in [json!(-0.0), json!("-0.0"), json!("-0")] {
            let number = finite_number(&zero).expect("zero is a valid amount");
            assert!(number == 0.0 && number.is_sign_positive());
        }

        let key = json!({"data": {"limit": 9e15, "usage": -9e15}});
        let snapshot = parse_openrouter_balances(&key, None).expect("bounded key amounts");
        assert_eq!(snapshot.balances[0].remaining, None);
    }

    #[test]
    fn serialized_snapshot_excludes_upstream_credentials_and_unknown_fields() {
        let key = json!({
            "data": {
                "usage": 0,
                "api_key": "private-test-value",
                "token": "private-test-value",
                "label": "private-test-value"
            }
        });

        let snapshot = parse_openrouter_balances(&key, None).expect("valid OpenRouter key");
        let encoded = serde_json::to_value(snapshot).expect("serialize snapshot");

        assert_eq!(encoded["providerId"], "openrouter");
        assert_eq!(encoded["status"], "available");
        assert!(!encoded.to_string().contains("private-test-value"));
        assert_eq!(encoded["balances"][0]["remaining"], Value::Null);
    }

    #[test]
    fn deepseek_reads_string_balances_and_availability() {
        let balance = json!({
            "is_available": false,
            "balance_infos": [
                {
                    "currency": "CNY",
                    "total_balance": "-0.50",
                    "granted_balance": "0",
                    "topped_up_balance": "-0.50"
                },
                {"currency": "USD", "total_balance": "2.50"}
            ]
        });

        let snapshot = parse_deepseek_balance(&balance).expect("valid DeepSeek balance");

        assert_eq!(snapshot.status, "unavailable");
        assert_eq!(snapshot.reason_code, Some("insufficient_balance"));
        assert_eq!(snapshot.balances[0].total, Some(-0.5));
        assert_eq!(snapshot.balances[1].total, Some(2.5));
    }

    #[test]
    fn deepseek_keeps_the_first_balance_for_a_repeated_currency() {
        let balance = json!({
            "balance_infos": [
                {"currency": "USD", "total_balance": "1.00"},
                {"currency": "USD", "total_balance": "9.00"}
            ]
        });

        let snapshot = parse_deepseek_balance(&balance).expect("valid DeepSeek balance");

        assert_eq!(snapshot.balances.len(), 1);
        assert_eq!(snapshot.balances[0].total, Some(1.0));
    }

    #[test]
    fn deepseek_rejects_malformed_balances() {
        let not_a_number = json!({
            "balance_infos": [{"currency": "USD", "total_balance": "NaN"}]
        });
        assert!(parse_deepseek_balance(&not_a_number).is_err());
        assert!(parse_deepseek_balance(&json!({})).is_err());
    }

    #[test]
    fn opencode_go_usage_reads_windows_and_reset_times() {
        let usage = json!({
            "usage": {
                "rolling": {"percent": 12.5, "resetsAt": "2026-11-18T12:00:00Z"},
                "weekly": {"percent": 30},
                "monthly": {"percent": 70}
            }
        });

        let snapshot = parse_opencode_go_usage(&usage).expect("valid OpenCode Go usage");

        assert_eq!(snapshot.windows.len(), 3);
        assert_eq!(snapshot.windows[0].window_seconds, Some(18_000));
        assert_eq!(
            snapshot.windows[0].reset_at_unix_ms,
            Some(1_795_003_200_000)
        );
        assert_eq!(snapshot.windows[2].window_seconds, None);
    }

    #[test]
    fn opencode_go_usage_keeps_a_window_with_an_invalid_reset_time() {
        let usage = json!({"usage": {"rolling": {"percent": 10, "resetsAt": "invalid"}}});

        let snapshot = parse_opencode_go_usage(&usage).expect("a window without a reset time");

        assert_eq!(snapshot.windows.len(), 1);
        assert_eq!(snapshot.windows[0].reset_at_unix_ms, None);
    }

    #[test]
    fn opencode_go_usage_rejects_missing_and_negative_percentages() {
        let missing = json!({"rolling": {"resetsAt": "2026-11-18T12:00:00Z"}});
        assert!(parse_opencode_go_usage(&missing).is_err());

        let negative = json!({"rolling": {"percent": -1}});
        assert!(parse_opencode_go_usage(&negative).is_err());
    }

    #[test]
    fn opencode_go_usage_ignores_a_null_usage_wrapper() {
        let usage = json!({"usage": null, "rolling": {"percent": 10}});

        let snapshot = parse_opencode_go_usage(&usage).expect("windows next to a null wrapper");

        assert_eq!(snapshot.windows.len(), 1);
        assert!(parse_opencode_go_usage(&json!({"usage": null})).is_err());
    }

    #[test]
    fn unsupported_opencode_go_endpoints_are_hidden_but_rejected_keys_are_not() {
        for status in [StatusCode::FORBIDDEN, StatusCode::NOT_FOUND] {
            assert!(matches!(http_error(status), FetchError::Unsupported(_)));
        }
        for status in [StatusCode::UNAUTHORIZED, StatusCode::BAD_GATEWAY] {
            assert!(matches!(http_error(status), FetchError::Failed(_)));
        }
    }

    #[test]
    fn api_keys_must_be_printable_ascii_without_spaces() {
        assert!(is_usable_api_key("sk-or-v1-abc_123"));
        for key in [
            "",
            " ",
            "bad key",
            "bad\nkey",
            "k\u{e9}y",
            &"k".repeat(4097),
        ] {
            assert!(!is_usable_api_key(key), "{key:?}");
        }
    }

    #[test]
    fn rate_limits_are_retryable_and_categorized() {
        let error = http_error(StatusCode::TOO_MANY_REQUESTS).into_error();

        assert_eq!(error.code, "rate_limited");
        assert_eq!(error.category.as_str(), "rate_limit");
        assert!(error.retryable);
    }

    #[test]
    fn http_errors_map_to_existing_codes_and_categories() {
        let cases = [
            (
                StatusCode::UNAUTHORIZED,
                "authentication_required",
                "authentication",
            ),
            (StatusCode::FORBIDDEN, "permission_denied", "authorization"),
            (StatusCode::NOT_FOUND, "provider_request_failed", "provider"),
            (
                StatusCode::BAD_GATEWAY,
                "provider_request_failed",
                "provider",
            ),
        ];

        for (status, code, category) in cases {
            let error = http_error(status).into_error();
            assert_eq!(error.code, code);
            assert_eq!(error.category.as_str(), category);
        }
        assert_eq!(network_error().category.as_str(), "network");
        assert_eq!(timeout_error().category.as_str(), "timeout");
        assert_eq!(invalid_response_error().category.as_str(), "provider");
    }
}
