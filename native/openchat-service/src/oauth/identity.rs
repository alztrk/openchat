use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::Value;
use zeroize::Zeroizing;

use crate::protocol::ServiceError;

use super::account_info_error;
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

pub(super) fn parse_identity(token: &str) -> Result<ParsedIdentity, ServiceError> {
    let claims = decode_id_token_claims(token)?;
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

fn decode_id_token_claims(token: &str) -> Result<Value, ServiceError> {
    let mut parts = token.split('.');
    let _header = parts.next().ok_or_else(account_info_error)?;
    let payload = parts.next().ok_or_else(account_info_error)?;
    if parts.next().is_none() || parts.next().is_some() {
        return Err(account_info_error());
    }
    let bytes = Zeroizing::new(
        URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| account_info_error())?,
    );
    serde_json::from_slice(&bytes).map_err(|_| account_info_error())
}

fn string_claim(object: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}
