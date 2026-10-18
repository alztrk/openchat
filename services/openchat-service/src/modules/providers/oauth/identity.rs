//! Validates OpenAI ID token claims and extracts the account identity used by the client.
//!
//! The token arrives over the configured HTTPS endpoint; issuer, audience, subject, authorized
//! party, and time claims are still validated before profile information is accepted.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

use crate::protocol::ServiceError;

use super::account_info_error;

const OPENAI_OIDC_ISSUER: &str = "https://auth.openai.com";
const ID_TOKEN_SIGNING_ALGORITHM: &str = "RS256";
const MAX_ID_TOKEN_ISSUED_AT_FUTURE_SKEW_SECONDS: u64 = 5 * 60;

pub(super) struct ExternalWorkspace {
    pub(super) id: String,
    pub(super) name: Option<String>,
    pub(super) plan_type: Option<String>,
    pub(super) is_default: bool,
}

pub(super) struct ParsedIdentity {
    pub(super) email: Option<String>,
    pub(super) display_name: Option<String>,
    pub(super) plan_type: Option<String>,
    pub(super) external_user_id: Option<String>,
    pub(super) workspaces: Vec<ExternalWorkspace>,
}

/// Extracts the account profile only after validating the OpenAI ID token claims.
///
/// The token is received directly from the configured HTTPS token endpoint with
/// certificate validation and redirects disabled. This transport trust replaces
/// local signature verification; issuer, audience, subject, and time claims are
/// still checked before any identity data is persisted.
pub(super) fn parse_identity(token: &str) -> Result<ParsedIdentity, ServiceError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| account_info_error())?
        .as_secs();
    parse_identity_at(token, now)
}

/// Validates and parses an ID token against a caller-provided Unix timestamp.
///
/// Taking `now` explicitly keeps expiration checks deterministic in tests.
fn parse_identity_at(token: &str, now: u64) -> Result<ParsedIdentity, ServiceError> {
    let claims = decode_id_token_claims(token)?;
    validate_id_token_claims(&claims, now)?;
    let auth = claims
        .get("https://api.openai.com/auth")
        .and_then(Value::as_object)
        .ok_or_else(account_info_error)?;
    let profile = claims
        .get("https://api.openai.com/profile")
        .and_then(Value::as_object);
    let external_user_id =
        string_claim(auth, "chatgpt_user_id").or_else(|| string_claim(auth, "user_id"));
    let account_id = string_claim(auth, "chatgpt_account_id");
    let email = profile
        .and_then(|value| value.get("email"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            claims
                .get("email")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
        })
        .map(str::trim)
        .map(str::to_owned);
    let display_name = profile
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let plan_type = string_claim(auth, "chatgpt_plan_type");
    let mut workspaces = Vec::new();
    if let Some(organizations) = auth.get("organizations").and_then(Value::as_array) {
        for organization in organizations {
            let object = organization.as_object().ok_or_else(account_info_error)?;
            let id = object
                .get("id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or_else(account_info_error)?
                .to_owned();
            workspaces.push(ExternalWorkspace {
                id,
                name: object
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                plan_type: object
                    .get("plan_type")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                is_default: object.get("is_default").and_then(Value::as_bool) == Some(true),
            });
        }
    }
    if workspaces.is_empty() {
        let id = account_id.ok_or_else(account_info_error)?;
        workspaces.push(ExternalWorkspace {
            id,
            name: None,
            plan_type: plan_type.clone(),
            is_default: true,
        });
    }
    if workspaces.len() > 1 {
        for workspace in &mut workspaces {
            workspace.is_default = false;
        }
    }
    Ok(ParsedIdentity {
        email,
        display_name,
        plan_type,
        external_user_id,
        workspaces,
    })
}

/// Decodes the compact JWT envelope without treating its payload as trusted.
fn decode_id_token_claims(token: &str) -> Result<Value, ServiceError> {
    let mut parts = token.split('.');
    let header = parts.next().ok_or_else(account_info_error)?;
    let payload = parts.next().ok_or_else(account_info_error)?;
    let signature = parts.next().ok_or_else(account_info_error)?;
    if header.is_empty() || payload.is_empty() || signature.is_empty() || parts.next().is_some() {
        return Err(account_info_error());
    }
    let header_bytes = Zeroizing::new(
        URL_SAFE_NO_PAD
            .decode(header)
            .map_err(|_| account_info_error())?,
    );
    let header: Value = serde_json::from_slice(&header_bytes).map_err(|_| account_info_error())?;
    if header.get("alg").and_then(Value::as_str) != Some(ID_TOKEN_SIGNING_ALGORITHM) {
        return Err(account_info_error());
    }
    URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| account_info_error())?;
    let bytes = Zeroizing::new(
        URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| account_info_error())?,
    );
    serde_json::from_slice(&bytes).map_err(|_| account_info_error())
}

/// Enforces OpenID Connect issuer, audience, subject, and token-time constraints.
fn validate_id_token_claims(claims: &Value, now: u64) -> Result<(), ServiceError> {
    if claims.get("iss").and_then(Value::as_str) != Some(OPENAI_OIDC_ISSUER) {
        return Err(account_info_error());
    }

    let audiences = match claims.get("aud") {
        Some(Value::String(audience)) => vec![audience.as_str()],
        Some(Value::Array(audiences)) if !audiences.is_empty() => audiences
            .iter()
            .map(Value::as_str)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(account_info_error)?,
        _ => return Err(account_info_error()),
    };
    if audiences.len() != 1 || audiences[0] != super::CLIENT_ID {
        return Err(account_info_error());
    }
    if claims
        .get("azp")
        .is_some_and(|authorized_party| authorized_party.as_str() != Some(super::CLIENT_ID))
    {
        return Err(account_info_error());
    }

    if claims
        .get("sub")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        return Err(account_info_error());
    }

    let expiration = claims
        .get("exp")
        .and_then(Value::as_u64)
        .ok_or_else(account_info_error)?;
    let issued_at = claims
        .get("iat")
        .and_then(Value::as_u64)
        .ok_or_else(account_info_error)?;
    if expiration <= now
        || issued_at > now.saturating_add(MAX_ID_TOKEN_ISSUED_AT_FUTURE_SKEW_SECONDS)
        || issued_at >= expiration
    {
        return Err(account_info_error());
    }

    Ok(())
}

fn string_claim(object: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::super::CLIENT_ID;
    use super::{
        ID_TOKEN_SIGNING_ALGORITHM, MAX_ID_TOKEN_ISSUED_AT_FUTURE_SKEW_SECONDS, OPENAI_OIDC_ISSUER,
        ParsedIdentity, parse_identity_at,
    };
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use serde_json::{Value, json};

    const NOW: u64 = 1_800_000_000;

    fn valid_claims() -> Value {
        json!({
            "iss": OPENAI_OIDC_ISSUER,
            "sub": "provider-subject",
            "aud": CLIENT_ID,
            "exp": NOW + 3600,
            "iat": NOW - 60,
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "workspace-1",
                "chatgpt_user_id": "user-1",
                "chatgpt_plan_type": "plus"
            },
            "https://api.openai.com/profile": {
                "email": "user@example.invalid",
                "name": "OpenChat User"
            }
        })
    }

    fn token_with_algorithm(claims: &Value, algorithm: &str) -> String {
        let header = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&json!({ "alg": algorithm, "typ": "JWT" }))
                .expect("serialize test JWT header"),
        );
        let payload =
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).expect("serialize test JWT claims"));
        let signature = URL_SAFE_NO_PAD.encode(b"test-signature");
        format!("{header}.{payload}.{signature}")
    }

    fn token(claims: &Value) -> String {
        token_with_algorithm(claims, ID_TOKEN_SIGNING_ALGORITHM)
    }

    fn parse_claims(claims: &Value) -> Result<ParsedIdentity, crate::protocol::ServiceError> {
        parse_identity_at(&token(claims), NOW)
    }

    #[test]
    fn parses_identity_from_valid_token_claims() {
        let identity = parse_claims(&valid_claims()).expect("valid ID token claims");

        assert_eq!(identity.email.as_deref(), Some("user@example.invalid"));
        assert_eq!(identity.display_name.as_deref(), Some("OpenChat User"));
        assert_eq!(identity.external_user_id.as_deref(), Some("user-1"));
        assert_eq!(identity.workspaces.len(), 1);
        assert_eq!(identity.workspaces[0].id, "workspace-1");
    }

    #[test]
    fn rejects_untrusted_issuer_and_audience() {
        let mut claims = valid_claims();
        claims["iss"] = json!("https://attacker.example");
        assert!(parse_claims(&claims).is_err());

        let mut claims = valid_claims();
        claims["aud"] = json!("another-client");
        assert!(parse_claims(&claims).is_err());

        let mut claims = valid_claims();
        claims["aud"] = json!([CLIENT_ID, "untrusted-client"]);
        assert!(parse_claims(&claims).is_err());
    }

    #[test]
    fn rejects_invalid_subject_and_expiration() {
        let mut claims = valid_claims();
        claims.as_object_mut().expect("claims object").remove("sub");
        assert!(parse_claims(&claims).is_err());

        let mut claims = valid_claims();
        claims["exp"] = json!(NOW);
        assert!(parse_claims(&claims).is_err());
    }

    #[test]
    fn rejects_missing_or_far_future_issue_time() {
        let mut claims = valid_claims();
        claims.as_object_mut().expect("claims object").remove("iat");
        assert!(parse_claims(&claims).is_err());

        let mut claims = valid_claims();
        claims["iat"] = json!(NOW + MAX_ID_TOKEN_ISSUED_AT_FUTURE_SKEW_SECONDS + 1);
        assert!(parse_claims(&claims).is_err());
    }

    #[test]
    fn rejects_wrong_authorized_party_and_signing_algorithm() {
        let mut claims = valid_claims();
        claims["azp"] = json!("another-client");
        assert!(parse_claims(&claims).is_err());

        let invalid_algorithm_token = token_with_algorithm(&valid_claims(), "none");
        assert!(parse_identity_at(&invalid_algorithm_token, NOW).is_err());
    }

    #[test]
    fn rejects_malformed_jwt_envelopes() {
        assert!(parse_identity_at("header.payload", NOW).is_err());
        assert!(parse_identity_at(".payload.signature", NOW).is_err());
        assert!(parse_identity_at("header.payload.signature.extra", NOW).is_err());
    }
}
