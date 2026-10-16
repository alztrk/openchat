use serde::{Deserialize, Serialize, Serializer};
use zeroize::{Zeroize, Zeroizing};

use crate::mcp_storage::{read_secret_map, write_secret_map, McpStorageError};

const CHATGPT_API_KEY_PREFIX: &str = "openchat.chatgpt.api_key.";
const COMPATIBLE_KEY_PREFIX: &str = "openchat.compatible_provider.api_key.";
const CUSTOM_PROVIDER_KEY_PREFIX: &str = "openchat.custom_provider.api_key.";
const CUSTOM_PROVIDER_CONFIG_PREFIX: &str = "openchat.custom_provider.config.";
const OPENCODE_KEY: &str = "openchat.opencode.console_api_key";
const MAXIMUM_KEY_BYTES: usize = 4096;
const COMPATIBLE_PROVIDER_IDS: [&str; 15] = [
    "gemini",
    "groq",
    "cerebras",
    "openrouter",
    "mistral",
    "deepseek",
    "xai",
    "perplexity",
    "together",
    "sambanova",
    "anthropic",
    "fireworks",
    "deepinfra",
    "dashscope",
    "cohere",
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStorageError {
    code: &'static str,
    message: &'static str,
}

impl ProviderStorageError {
    fn invalid() -> Self {
        Self {
            code: "provider_credential_invalid",
            message: "The provider credential is invalid.",
        }
    }

    fn unavailable() -> Self {
        Self {
            code: "provider_credential_storage_unavailable",
            message: "Saved provider credentials could not be accessed.",
        }
    }

    fn duplicate() -> Self {
        Self {
            code: "provider_credential_duplicate",
            message: "This ChatGPT API key is already saved.",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialSnapshot {
    chatgpt_api_keys: Vec<ChatGptApiKeySummary>,
    compatible_providers: Vec<CompatibleProviderSummary>,
    custom_providers: Vec<CustomProviderSummary>,
    open_code_configured: bool,
    open_code_key_suffix: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomProviderConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub protocol: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomProviderSummary {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub protocol: String,
    pub is_configured: bool,
    pub key_suffix: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatGptApiKeySummary {
    id: String,
    key_suffix: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompatibleProviderSummary {
    provider_id: String,
    is_configured: bool,
    key_suffix: Option<String>,
}

pub struct SecretValue(Zeroizing<String>);

impl Serialize for SecretValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.0.as_str())
    }
}

#[tauri::command]
pub fn load_provider_credential_status() -> Result<ProviderCredentialSnapshot, ProviderStorageError>
{
    let values = read_secret_map().map_err(map_storage_error)?;
    let mut chatgpt_api_keys = values
        .0
        .iter()
        .filter_map(|(key, value)| {
            let id = key.strip_prefix(CHATGPT_API_KEY_PREFIX)?;
            if !is_chatgpt_connection_id(id) || value.trim().is_empty() {
                return None;
            }
            Some(ChatGptApiKeySummary {
                id: id.to_owned(),
                key_suffix: key_suffix(value),
            })
        })
        .collect::<Vec<_>>();
    chatgpt_api_keys.sort_by(|left, right| left.id.cmp(&right.id));

    let compatible_providers = COMPATIBLE_PROVIDER_IDS
        .iter()
        .map(|provider_id| {
            let value = values
                .0
                .get(&format!("{COMPATIBLE_KEY_PREFIX}{provider_id}"));
            CompatibleProviderSummary {
                provider_id: (*provider_id).to_owned(),
                is_configured: value.is_some_and(|value| !value.trim().is_empty()),
                key_suffix: value.and_then(|value| key_suffix(value)),
            }
        })
        .collect();

    let open_code_key = values.0.get(OPENCODE_KEY);
    let custom_providers = load_custom_providers(&values)?;
    Ok(ProviderCredentialSnapshot {
        chatgpt_api_keys,
        compatible_providers,
        custom_providers,
        open_code_configured: open_code_key.is_some_and(|value| !value.trim().is_empty()),
        open_code_key_suffix: open_code_key.and_then(|value| key_suffix(value)),
    })
}

#[tauri::command]
pub fn save_chatgpt_api_key(
    connection_id: String,
    api_key: String,
) -> Result<(), ProviderStorageError> {
    if !is_chatgpt_connection_id(&connection_id) {
        return Err(ProviderStorageError::invalid());
    }
    let api_key = validated_key("chatgpt_api", api_key)?;
    let mut values = read_secret_map().map_err(map_storage_error)?;
    if values
        .0
        .iter()
        .any(|(name, value)| name.starts_with(CHATGPT_API_KEY_PREFIX) && value == api_key.as_str())
    {
        return Err(ProviderStorageError::duplicate());
    }
    values.0.insert(
        format!("{CHATGPT_API_KEY_PREFIX}{connection_id}"),
        api_key.to_string(),
    );
    write_secret_map(&values).map_err(map_storage_error)
}

#[tauri::command]
pub fn save_compatible_provider_api_key(
    provider_id: String,
    api_key: String,
) -> Result<(), ProviderStorageError> {
    if !COMPATIBLE_PROVIDER_IDS.contains(&provider_id.as_str()) {
        return Err(ProviderStorageError::invalid());
    }
    let api_key = validated_key(&provider_id, api_key)?;
    let mut values = read_secret_map().map_err(map_storage_error)?;
    values.0.insert(
        format!("{COMPATIBLE_KEY_PREFIX}{provider_id}"),
        api_key.to_string(),
    );
    write_secret_map(&values).map_err(map_storage_error)
}

#[tauri::command]
pub fn save_opencode_api_key(api_key: String) -> Result<(), ProviderStorageError> {
    let api_key = validated_key("opencode", api_key)?;
    let mut values = read_secret_map().map_err(map_storage_error)?;
    values
        .0
        .insert(OPENCODE_KEY.to_owned(), api_key.to_string());
    write_secret_map(&values).map_err(map_storage_error)
}

#[tauri::command]
pub fn save_custom_provider(
    id: String,
    name: String,
    base_url: String,
    protocol: String,
    api_key: Option<String>,
) -> Result<CustomProviderSummary, ProviderStorageError> {
    let config = validated_custom_provider(id, name, base_url, protocol)?;
    let mut values = read_secret_map().map_err(map_storage_error)?;
    if let Some(api_key) = api_key {
        if !api_key.trim().is_empty() {
            let api_key = validated_key(&config.id, api_key)?;
            values.0.insert(
                format!("{CUSTOM_PROVIDER_KEY_PREFIX}{}", config.id),
                api_key.to_string(),
            );
        }
    }
    let serialized =
        serde_json::to_string(&config).map_err(|_| ProviderStorageError::unavailable())?;
    values.0.insert(
        format!("{CUSTOM_PROVIDER_CONFIG_PREFIX}{}", config.id),
        serialized,
    );
    write_secret_map(&values).map_err(map_storage_error)?;
    let summary = custom_provider_summary(&config, &values);
    Ok(summary)
}

#[tauri::command]
pub fn delete_custom_provider(id: String) -> Result<(), ProviderStorageError> {
    if !is_custom_provider_id(&id) {
        return Err(ProviderStorageError::invalid());
    }
    let mut values = read_secret_map().map_err(map_storage_error)?;
    values
        .0
        .remove(&format!("{CUSTOM_PROVIDER_KEY_PREFIX}{id}"));
    values
        .0
        .remove(&format!("{CUSTOM_PROVIDER_CONFIG_PREFIX}{id}"));
    write_secret_map(&values).map_err(map_storage_error)
}

#[tauri::command]
pub fn get_provider_api_key(
    provider_id: String,
    connection_id: Option<String>,
) -> Result<Option<SecretValue>, ProviderStorageError> {
    let storage_key = match provider_id.as_str() {
        "chatgpt_api" => {
            let connection_id = connection_id.ok_or_else(ProviderStorageError::invalid)?;
            if !is_chatgpt_connection_id(&connection_id) {
                return Err(ProviderStorageError::invalid());
            }
            format!("{CHATGPT_API_KEY_PREFIX}{connection_id}")
        }
        "opencode" => OPENCODE_KEY.to_owned(),
        provider_id if is_custom_provider_id(provider_id) => {
            let values = read_secret_map().map_err(map_storage_error)?;
            if !values
                .0
                .contains_key(&format!("{CUSTOM_PROVIDER_CONFIG_PREFIX}{provider_id}"))
            {
                return Err(ProviderStorageError::invalid());
            }
            let key = format!("{CUSTOM_PROVIDER_KEY_PREFIX}{provider_id}");
            return Ok(values.0.get(&key).and_then(|value| {
                (!value.trim().is_empty()).then(|| SecretValue(Zeroizing::new(value.clone())))
            }));
        }
        provider_id if COMPATIBLE_PROVIDER_IDS.contains(&provider_id) => {
            format!("{COMPATIBLE_KEY_PREFIX}{provider_id}")
        }
        _ => return Err(ProviderStorageError::invalid()),
    };
    let values = read_secret_map().map_err(map_storage_error)?;
    Ok(values.0.get(&storage_key).and_then(|value| {
        (!value.trim().is_empty()).then(|| SecretValue(Zeroizing::new(value.clone())))
    }))
}

#[tauri::command]
pub fn delete_chatgpt_api_key(connection_id: String) -> Result<(), ProviderStorageError> {
    if !is_chatgpt_connection_id(&connection_id) {
        return Err(ProviderStorageError::invalid());
    }
    let mut values = read_secret_map().map_err(map_storage_error)?;
    values
        .0
        .remove(&format!("{CHATGPT_API_KEY_PREFIX}{connection_id}"));
    write_secret_map(&values).map_err(map_storage_error)
}

#[tauri::command]
pub fn delete_compatible_provider_api_key(provider_id: String) -> Result<(), ProviderStorageError> {
    if !COMPATIBLE_PROVIDER_IDS.contains(&provider_id.as_str()) {
        return Err(ProviderStorageError::invalid());
    }
    let mut values = read_secret_map().map_err(map_storage_error)?;
    values
        .0
        .remove(&format!("{COMPATIBLE_KEY_PREFIX}{provider_id}"));
    write_secret_map(&values).map_err(map_storage_error)
}

#[tauri::command]
pub fn delete_opencode_api_key() -> Result<(), ProviderStorageError> {
    let mut values = read_secret_map().map_err(map_storage_error)?;
    values.0.remove(OPENCODE_KEY);
    write_secret_map(&values).map_err(map_storage_error)
}

fn validated_key(
    provider_id: &str,
    mut value: String,
) -> Result<Zeroizing<String>, ProviderStorageError> {
    let normalized = value.trim().to_owned();
    value.zeroize();
    let value = Zeroizing::new(normalized);
    if value.is_empty()
        || value.len() > MAXIMUM_KEY_BYTES
        || !value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    {
        return Err(ProviderStorageError::invalid());
    }
    let valid_prefix = match provider_id {
        "chatgpt_api" => value.starts_with("sk-") && value.len() > 3,
        "gemini" => {
            (value.starts_with("AIza") && value.len() > 4)
                || (value.starts_with("AQ.") && value.len() > 3)
        }
        "groq" => value.starts_with("gsk_") && value.len() > 4,
        "openrouter" => value.starts_with("sk-or-v1-") && value.len() > 9,
        _ => true,
    };
    if !valid_prefix {
        return Err(ProviderStorageError::invalid());
    }
    Ok(value)
}

fn validated_custom_provider(
    id: String,
    name: String,
    base_url: String,
    protocol: String,
) -> Result<CustomProviderConfig, ProviderStorageError> {
    let name = name.trim();
    let base_url = normalized_base_url(&base_url).ok_or_else(ProviderStorageError::invalid)?;
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err(ProviderStorageError::invalid());
    }
    if !matches!(protocol.as_str(), "openAi" | "anthropic") {
        return Err(ProviderStorageError::invalid());
    }
    if !is_custom_provider_id(&id) {
        return Err(ProviderStorageError::invalid());
    }
    Ok(CustomProviderConfig {
        id,
        name: name.to_owned(),
        base_url,
        protocol,
    })
}

fn normalized_base_url(value: &str) -> Option<String> {
    let mut url = url::Url::parse(value.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || value.trim().len() > 2048
    {
        return None;
    }
    let path = url.path().trim_end_matches('/').to_owned();
    url.set_path(&path);
    Some(url.to_string().trim_end_matches('/').to_owned())
}

fn is_custom_provider_id(value: &str) -> bool {
    (19..=64).contains(&value.len())
        && value.starts_with("custom_")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn load_custom_providers(
    values: &crate::mcp_storage::SecretMap,
) -> Result<Vec<CustomProviderSummary>, ProviderStorageError> {
    let mut providers = values
        .0
        .iter()
        .filter_map(|(key, value)| {
            let id = key.strip_prefix(CUSTOM_PROVIDER_CONFIG_PREFIX)?;
            Some((id, value))
        })
        .map(|(id, value)| {
            let config = serde_json::from_str::<CustomProviderConfig>(value)
                .map_err(|_| ProviderStorageError::invalid())?;
            if config.id != id {
                return Err(ProviderStorageError::invalid());
            }
            let validated = validated_custom_provider(
                config.id,
                config.name,
                config.base_url,
                config.protocol,
            )?;
            Ok(custom_provider_summary(&validated, values))
        })
        .collect::<Result<Vec<_>, ProviderStorageError>>()?;
    providers.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(providers)
}

fn custom_provider_summary(
    config: &CustomProviderConfig,
    values: &crate::mcp_storage::SecretMap,
) -> CustomProviderSummary {
    let key = values
        .0
        .get(&format!("{CUSTOM_PROVIDER_KEY_PREFIX}{}", config.id));
    CustomProviderSummary {
        id: config.id.clone(),
        name: config.name.clone(),
        base_url: config.base_url.clone(),
        protocol: config.protocol.clone(),
        is_configured: key.is_some_and(|value| !value.trim().is_empty()),
        key_suffix: key.and_then(|value| key_suffix(value)),
    }
}

fn is_chatgpt_connection_id(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn key_suffix(value: &str) -> Option<String> {
    let suffix = value.chars().rev().take(4).collect::<String>();
    (suffix.chars().count() == 4).then(|| suffix.chars().rev().collect())
}

fn map_storage_error(_: McpStorageError) -> ProviderStorageError {
    ProviderStorageError::unavailable()
}
