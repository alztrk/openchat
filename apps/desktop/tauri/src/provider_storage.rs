//! Validates and stores provider credentials through the encrypted secret-store module.
//!
//! Frontend commands expose only configuration status and key suffixes; full credential values
//! are never returned over IPC.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use zeroize::{Zeroize, Zeroizing};

use crate::{
    mcp_storage::{lock_secret_map, read_secret_map, write_secret_map, McpStorageError},
    request_bounds::BoundedText,
};

const CHATGPT_API_KEY_PREFIX: &str = "openchat.chatgpt.api_key.";
const COMPATIBLE_KEY_PREFIX: &str = "openchat.compatible_provider.api_key.";
const CUSTOM_PROVIDER_KEY_PREFIX: &str = "openchat.custom_provider.api_key.";
const CUSTOM_PROVIDER_CONFIG_PREFIX: &str = "openchat.custom_provider.config.";
const OPENCODE_KEY: &str = "openchat.opencode.console_api_key";
const MAXIMUM_KEY_BYTES: usize = 4096;
const MAXIMUM_PROVIDER_ID_BYTES: usize = 64;
const MAXIMUM_PROVIDER_NAME_BYTES: usize = 320;
const MAXIMUM_PROVIDER_URL_BYTES: usize = 2048;
const MAXIMUM_PROVIDER_PROTOCOL_BYTES: usize = 16;
const MAXIMUM_LOCALE_BYTES: usize = 32;
const COMPATIBLE_PROVIDER_IDS: [&str; 16] = [
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
    "kilo",
];
static HTTP_PROVIDER_CONFIRMATION_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Safe error contract for provider credential operations.
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

    fn key_required_for_endpoint_change() -> Self {
        Self {
            code: "custom_provider_key_required_for_endpoint_change",
            message:
                "Enter the custom provider API key again after changing its endpoint or API format.",
        }
    }

    fn http_approval_required() -> Self {
        Self {
            code: "custom_provider_http_approval_required",
            message: "Explicit approval is required for an unencrypted HTTP provider.",
        }
    }

    fn http_approval_denied() -> Self {
        Self {
            code: "custom_provider_http_approval_denied",
            message: "The unencrypted HTTP provider was not saved because approval was denied.",
        }
    }

    fn confirmation_unavailable() -> Self {
        Self {
            code: "provider_confirmation_unavailable",
            message:
                "The operating-system confirmation is unavailable. The provider was not saved.",
        }
    }

    fn confirmation_in_progress() -> Self {
        Self {
            code: "provider_confirmation_in_progress",
            message: "Another provider security confirmation is already open.",
        }
    }
}

/// Prevents overlapping native dialogs for insecure provider configurations.
struct HttpProviderConfirmationGuard;

impl HttpProviderConfirmationGuard {
    /// Acquires the single-flight guard before opening an OS-owned confirmation.
    fn acquire() -> Result<Self, ProviderStorageError> {
        HTTP_PROVIDER_CONFIRMATION_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self)
            .map_err(|_| ProviderStorageError::confirmation_in_progress())
    }
}

impl Drop for HttpProviderConfirmationGuard {
    fn drop(&mut self) {
        HTTP_PROVIDER_CONFIRMATION_ACTIVE.store(false, Ordering::Release);
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Provider setup status returned without exposing full credential values.
pub struct ProviderCredentialSnapshot {
    chatgpt_api_keys: Vec<ChatGptApiKeySummary>,
    compatible_providers: Vec<CompatibleProviderSummary>,
    custom_providers: Vec<CustomProviderSummary>,
    open_code_configured: bool,
    open_code_key_suffix: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// Connection settings for a user-defined OpenAI-compatible provider.
pub struct CustomProviderConfig {
    /// Stable identifier used to associate this configuration with its credential.
    pub id: String,
    /// User-visible provider name.
    pub name: String,
    /// HTTP(S) API base URL without embedded credentials, query, or fragment.
    pub base_url: String,
    /// Supported request protocol, currently `openAi` or `anthropic`.
    pub protocol: String,
    /// Explicit user approval for sending provider requests over unencrypted HTTP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_insecure_http: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
/// Bounded renderer input for creating or editing a custom provider.
pub(crate) struct SaveCustomProviderInput {
    id: BoundedText<MAXIMUM_PROVIDER_ID_BYTES>,
    name: BoundedText<MAXIMUM_PROVIDER_NAME_BYTES>,
    base_url: BoundedText<MAXIMUM_PROVIDER_URL_BYTES>,
    protocol: BoundedText<MAXIMUM_PROVIDER_PROTOCOL_BYTES>,
    allow_insecure_http: bool,
    api_key: Option<BoundedText<MAXIMUM_KEY_BYTES>>,
    locale: BoundedText<MAXIMUM_LOCALE_BYTES>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Validated custom-provider settings and non-secret credential status.
pub struct CustomProviderSummary {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub protocol: String,
    pub allow_insecure_http: bool,
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

#[tauri::command]
/// Loads provider setup state and redacted credential suffixes for settings UI.
///
/// # Errors
/// Returns a storage error if the encrypted credential store cannot be read or contains
/// a custom provider configuration that fails validation.
pub fn load_provider_credential_status() -> Result<ProviderCredentialSnapshot, ProviderStorageError>
{
    let values = read_secret_map().map_err(map_storage_error)?;
    let mut chatgpt_api_keys = values
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
            let value = values.get(&format!("{COMPATIBLE_KEY_PREFIX}{provider_id}"));
            CompatibleProviderSummary {
                provider_id: (*provider_id).to_owned(),
                is_configured: value.is_some_and(|value| !value.trim().is_empty()),
                key_suffix: value.and_then(key_suffix),
            }
        })
        .collect();

    let open_code_key = values.get(OPENCODE_KEY);
    let custom_providers = load_custom_providers(&values)?;
    Ok(ProviderCredentialSnapshot {
        chatgpt_api_keys,
        compatible_providers,
        custom_providers,
        open_code_configured: open_code_key.is_some_and(|value| !value.trim().is_empty()),
        open_code_key_suffix: open_code_key.and_then(key_suffix),
    })
}

#[tauri::command]
/// Saves a validated API key for one ChatGPT API connection.
///
/// The key is normalized, validated, and persisted under the supplied connection ID. A key
/// already saved for another connection is rejected to avoid ambiguous duplicate credentials.
/// Connection IDs are limited to 64 UTF-8 bytes and API keys to 4,096 UTF-8 bytes during IPC
/// argument deserialization.
///
/// # Errors
/// Returns a validation, duplicate-key, or secure-storage error without returning the key.
pub fn save_chatgpt_api_key(
    connection_id: BoundedText<MAXIMUM_PROVIDER_ID_BYTES>,
    api_key: BoundedText<MAXIMUM_KEY_BYTES>,
) -> Result<(), ProviderStorageError> {
    let connection_id = connection_id.into_inner();
    let api_key = Zeroizing::new(api_key.into_inner());
    if !is_chatgpt_connection_id(&connection_id) {
        return Err(ProviderStorageError::invalid());
    }
    let api_key = validated_key("chatgpt_api", api_key)?;
    let _update_lock = lock_secret_map().map_err(map_storage_error)?;
    let mut values = read_secret_map().map_err(map_storage_error)?;
    if values
        .iter()
        .any(|(name, value)| name.starts_with(CHATGPT_API_KEY_PREFIX) && value == api_key.as_str())
    {
        return Err(ProviderStorageError::duplicate());
    }
    values.insert(
        format!("{CHATGPT_API_KEY_PREFIX}{connection_id}"),
        api_key.to_string(),
    );
    write_secret_map(&values, &_update_lock).map_err(map_storage_error)
}

#[tauri::command]
/// Saves an API key for a supported compatible provider.
///
/// Provider IDs are limited to 64 UTF-8 bytes and API keys to 4,096 UTF-8 bytes during IPC
/// argument deserialization.
///
/// # Errors
/// Returns a validation or secure-storage error without returning the key.
pub fn save_compatible_provider_api_key(
    provider_id: BoundedText<MAXIMUM_PROVIDER_ID_BYTES>,
    api_key: BoundedText<MAXIMUM_KEY_BYTES>,
) -> Result<(), ProviderStorageError> {
    let provider_id = provider_id.into_inner();
    let api_key = Zeroizing::new(api_key.into_inner());
    if !COMPATIBLE_PROVIDER_IDS.contains(&provider_id.as_str()) {
        return Err(ProviderStorageError::invalid());
    }
    let api_key = validated_key(&provider_id, api_key)?;
    let _update_lock = lock_secret_map().map_err(map_storage_error)?;
    let mut values = read_secret_map().map_err(map_storage_error)?;
    values.insert(
        format!("{COMPATIBLE_KEY_PREFIX}{provider_id}"),
        api_key.to_string(),
    );
    write_secret_map(&values, &_update_lock).map_err(map_storage_error)
}

#[tauri::command]
/// Saves the OpenCode console API key after provider-specific validation.
///
/// API keys are limited to 4,096 UTF-8 bytes during IPC argument deserialization.
///
/// # Errors
/// Returns a validation or secure-storage error without returning the key.
pub fn save_opencode_api_key(
    api_key: BoundedText<MAXIMUM_KEY_BYTES>,
) -> Result<(), ProviderStorageError> {
    let api_key = validated_key("opencode", Zeroizing::new(api_key.into_inner()))?;
    let _update_lock = lock_secret_map().map_err(map_storage_error)?;
    let mut values = read_secret_map().map_err(map_storage_error)?;
    values.insert(OPENCODE_KEY.to_owned(), api_key.to_string());
    write_secret_map(&values, &_update_lock).map_err(map_storage_error)
}

#[tauri::command]
/// Creates or updates a custom provider and optionally replaces its API key.
///
/// An absent or blank key leaves a previously saved key unchanged only when the endpoint and API
/// format are unchanged. Changing either requires a newly supplied key so a saved credential is
/// never silently sent to a different destination. The returned summary contains only whether a
/// key exists and its last four characters.
/// Provider IDs, display names, base URLs, protocols, and optional API keys are limited to 64,
/// 320, 2,048, 16, and 4,096 UTF-8 bytes respectively during IPC argument deserialization.
///
/// # Errors
/// Returns a validation, key-reentry, or secure-storage error; the full key is never returned.
pub async fn save_custom_provider(
    app: AppHandle,
    input: SaveCustomProviderInput,
) -> Result<CustomProviderSummary, ProviderStorageError> {
    let api_key = input
        .api_key
        .map(|value| Zeroizing::new(value.into_inner()));
    let config = validated_custom_provider(
        input.id.into_inner(),
        input.name.into_inner(),
        input.base_url.into_inner(),
        input.protocol.into_inner(),
        Some(input.allow_insecure_http),
    )?;
    if requires_native_http_approval(&config) {
        let _confirmation_guard = HttpProviderConfirmationGuard::acquire()?;
        if !confirm_insecure_http_provider(app, config.base_url.clone(), input.locale.into_inner())
            .await?
        {
            return Err(ProviderStorageError::http_approval_denied());
        }
    }
    let _update_lock = lock_secret_map().map_err(map_storage_error)?;
    let mut values = read_secret_map().map_err(map_storage_error)?;
    let summary = update_custom_provider_values(&mut values, config, api_key)?;
    write_secret_map(&values, &_update_lock).map_err(map_storage_error)?;
    Ok(summary)
}

/// Shows an OS-owned confirmation before allowing a custom API key or chat content over HTTP.
async fn confirm_insecure_http_provider(
    app: AppHandle,
    base_url: String,
    locale: String,
) -> Result<bool, ProviderStorageError> {
    let copy = http_provider_dialog_copy(&locale);
    let message = format!("{}\n\n{}\n\n{}", copy.intro, base_url, copy.warning);
    let title = copy.title.to_owned();
    let buttons =
        MessageDialogButtons::OkCancelCustom(copy.allow.to_owned(), copy.cancel.to_owned());
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .message(message)
            .title(title)
            .buttons(buttons)
            .blocking_show()
    })
    .await
    .map_err(|_| ProviderStorageError::confirmation_unavailable())
}

/// Returns true when persisting an HTTP endpoint must follow native user confirmation.
fn requires_native_http_approval(config: &CustomProviderConfig) -> bool {
    config.base_url.starts_with("http://") && config.allow_insecure_http == Some(true)
}

/// Supplies fixed native-dialog copy for the supported application locales.
fn http_provider_dialog_copy(locale: &str) -> HttpProviderDialogCopy {
    match locale.split(['-', '_']).next() {
        Some("tr") => HttpProviderDialogCopy {
            title: "OpenChat güvenlik onayı",
            intro: "Bu özel sağlayıcı şifrelenmeyen HTTP kullanıyor:",
            warning: "Sohbet içeriği ve API anahtarları ağda okunabilir. Yalnızca güvendiğiniz yerel bir sağlayıcıysa devam edin.",
            allow: "HTTP'ye izin ver",
            cancel: "Vazgeç",
        },
        Some("de") => HttpProviderDialogCopy {
            title: "OpenChat-Sicherheitsbestätigung",
            intro: "Dieser benutzerdefinierte Anbieter verwendet unverschlüsseltes HTTP:",
            warning: "Chat-Inhalte und API-Schlüssel können im Netzwerk sichtbar sein. Fahren Sie nur bei einem vertrauenswürdigen lokalen Anbieter fort.",
            allow: "HTTP erlauben",
            cancel: "Abbrechen",
        },
        Some("es") => HttpProviderDialogCopy {
            title: "Confirmación de seguridad de OpenChat",
            intro: "Este proveedor personalizado usa HTTP sin cifrar:",
            warning: "El contenido del chat y las claves API pueden verse en la red. Continúa solo si es un proveedor local de confianza.",
            allow: "Permitir HTTP",
            cancel: "Cancelar",
        },
        Some("fr") => HttpProviderDialogCopy {
            title: "Confirmation de sécurité OpenChat",
            intro: "Ce fournisseur personnalisé utilise HTTP sans chiffrement :",
            warning: "Le contenu des discussions et les clés API peuvent être visibles sur le réseau. Continuez uniquement avec un fournisseur local de confiance.",
            allow: "Autoriser HTTP",
            cancel: "Annuler",
        },
        _ => HttpProviderDialogCopy {
            title: "OpenChat security confirmation",
            intro: "This custom provider uses unencrypted HTTP:",
            warning: "Chat content and API keys may be visible on the network. Continue only for a trusted local provider.",
            allow: "Allow HTTP",
            cancel: "Cancel",
        },
    }
}

/// Static wording used by the operating-system HTTP security confirmation.
struct HttpProviderDialogCopy {
    title: &'static str,
    intro: &'static str,
    warning: &'static str,
    allow: &'static str,
    cancel: &'static str,
}

/// Updates one provider in a decrypted map after validating that an existing key remains bound
/// to the same normalized endpoint and API format.
fn update_custom_provider_values(
    values: &mut crate::mcp_storage::SecretMap,
    config: CustomProviderConfig,
    api_key: Option<Zeroizing<String>>,
) -> Result<CustomProviderSummary, ProviderStorageError> {
    let api_key = api_key.filter(|value| !value.trim().is_empty());
    let key_name = format!("{CUSTOM_PROVIDER_KEY_PREFIX}{}", config.id);
    let has_existing_key = values
        .get(&key_name)
        .is_some_and(|value| !value.trim().is_empty());

    if has_existing_key
        && api_key.is_none()
        && !stored_custom_provider_matches_destination(values, &config)
    {
        return Err(ProviderStorageError::key_required_for_endpoint_change());
    }

    let serialized =
        serde_json::to_string(&config).map_err(|_| ProviderStorageError::unavailable())?;
    let validated_api_key = api_key
        .map(|value| validated_key(&config.id, value))
        .transpose()?;

    if let Some(api_key) = validated_api_key {
        values.insert(key_name, api_key.to_string());
    }
    values.insert(
        format!("{CUSTOM_PROVIDER_CONFIG_PREFIX}{}", config.id),
        serialized,
    );
    Ok(custom_provider_summary(&config, values))
}

/// Confirms that a stored key's current configuration parses and resolves to the requested
/// normalized endpoint and authentication protocol.
fn stored_custom_provider_matches_destination(
    values: &crate::mcp_storage::SecretMap,
    requested: &CustomProviderConfig,
) -> bool {
    let config_name = format!("{CUSTOM_PROVIDER_CONFIG_PREFIX}{}", requested.id);
    let Some(serialized) = values.get(&config_name) else {
        return false;
    };
    let Ok(stored) = serde_json::from_str::<CustomProviderConfig>(serialized) else {
        return false;
    };
    if stored.id != requested.id {
        return false;
    }
    let Ok(stored) = validated_custom_provider(
        stored.id,
        stored.name,
        stored.base_url,
        stored.protocol,
        stored.allow_insecure_http,
    ) else {
        return false;
    };
    stored.base_url == requested.base_url && stored.protocol == requested.protocol
}

#[tauri::command]
/// Deletes a custom provider's settings and associated API key.
///
/// # Errors
/// Returns a validation or secure-storage error.
pub fn delete_custom_provider(
    id: BoundedText<MAXIMUM_PROVIDER_ID_BYTES>,
) -> Result<(), ProviderStorageError> {
    let id = id.into_inner();
    if !is_custom_provider_id(&id) {
        return Err(ProviderStorageError::invalid());
    }
    let _update_lock = lock_secret_map().map_err(map_storage_error)?;
    let mut values = read_secret_map().map_err(map_storage_error)?;
    values.remove(&format!("{CUSTOM_PROVIDER_KEY_PREFIX}{id}"));
    values.remove(&format!("{CUSTOM_PROVIDER_CONFIG_PREFIX}{id}"));
    write_secret_map(&values, &_update_lock).map_err(map_storage_error)
}

#[tauri::command]
/// Deletes the saved API key for a ChatGPT API connection.
///
/// # Errors
/// Returns a validation or secure-storage error.
pub fn delete_chatgpt_api_key(
    connection_id: BoundedText<MAXIMUM_PROVIDER_ID_BYTES>,
) -> Result<(), ProviderStorageError> {
    let connection_id = connection_id.into_inner();
    if !is_chatgpt_connection_id(&connection_id) {
        return Err(ProviderStorageError::invalid());
    }
    let _update_lock = lock_secret_map().map_err(map_storage_error)?;
    let mut values = read_secret_map().map_err(map_storage_error)?;
    values.remove(&format!("{CHATGPT_API_KEY_PREFIX}{connection_id}"));
    write_secret_map(&values, &_update_lock).map_err(map_storage_error)
}

#[tauri::command]
/// Deletes the saved API key for a supported compatible provider.
///
/// # Errors
/// Returns a validation or secure-storage error.
pub fn delete_compatible_provider_api_key(
    provider_id: BoundedText<MAXIMUM_PROVIDER_ID_BYTES>,
) -> Result<(), ProviderStorageError> {
    let provider_id = provider_id.into_inner();
    if !COMPATIBLE_PROVIDER_IDS.contains(&provider_id.as_str()) {
        return Err(ProviderStorageError::invalid());
    }
    let _update_lock = lock_secret_map().map_err(map_storage_error)?;
    let mut values = read_secret_map().map_err(map_storage_error)?;
    values.remove(&format!("{COMPATIBLE_KEY_PREFIX}{provider_id}"));
    write_secret_map(&values, &_update_lock).map_err(map_storage_error)
}

#[tauri::command]
/// Deletes the saved OpenCode console API key.
///
/// # Errors
/// Returns a secure-storage error.
pub fn delete_opencode_api_key() -> Result<(), ProviderStorageError> {
    let _update_lock = lock_secret_map().map_err(map_storage_error)?;
    let mut values = read_secret_map().map_err(map_storage_error)?;
    values.remove(OPENCODE_KEY);
    write_secret_map(&values, &_update_lock).map_err(map_storage_error)
}

/// Trims, bounds, and provider-checks a credential while keeping the owned value zeroizing.
fn validated_key(
    provider_id: &str,
    mut value: Zeroizing<String>,
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
        "copilot" => false,
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

/// Validates custom provider identity, display name, endpoint, and protocol.
fn validated_custom_provider(
    id: String,
    name: String,
    base_url: String,
    protocol: String,
    allow_insecure_http: Option<bool>,
) -> Result<CustomProviderConfig, ProviderStorageError> {
    let name = name.trim();
    let base_url = normalized_base_url(&base_url).ok_or_else(ProviderStorageError::invalid)?;
    let uses_http = base_url.starts_with("http://");
    if uses_http && allow_insecure_http == Some(false) {
        return Err(ProviderStorageError::http_approval_required());
    }
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
        allow_insecure_http: if uses_http {
            allow_insecure_http
        } else {
            Some(false)
        },
    })
}

/// Normalizes an HTTP(S) endpoint while rejecting embedded credentials and URL decorations.
fn normalized_base_url(value: &str) -> Option<String> {
    let mut url = url::Url::parse(value.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || value.trim().len() > MAXIMUM_PROVIDER_URL_BYTES
    {
        return None;
    }
    let path = url.path().trim_end_matches('/').to_owned();
    url.set_path(&path);
    Some(url.to_string().trim_end_matches('/').to_owned())
}

/// Accepts only application-generated custom provider IDs.
fn is_custom_provider_id(value: &str) -> bool {
    (19..=64).contains(&value.len())
        && value.starts_with("custom_")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Deserializes and revalidates persisted custom provider configuration before exposing it.
fn load_custom_providers(
    values: &crate::mcp_storage::SecretMap,
) -> Result<Vec<CustomProviderSummary>, ProviderStorageError> {
    let mut providers = values
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
                config.allow_insecure_http,
            )?;
            Ok(custom_provider_summary(&validated, values))
        })
        .collect::<Result<Vec<_>, ProviderStorageError>>()?;
    providers.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(providers)
}

/// Builds the frontend-safe summary without copying the full API key into the response.
fn custom_provider_summary(
    config: &CustomProviderConfig,
    values: &crate::mcp_storage::SecretMap,
) -> CustomProviderSummary {
    let key = values.get(&format!("{CUSTOM_PROVIDER_KEY_PREFIX}{}", config.id));
    CustomProviderSummary {
        id: config.id.clone(),
        name: config.name.clone(),
        base_url: config.base_url.clone(),
        protocol: config.protocol.clone(),
        allow_insecure_http: config.allow_insecure_http.unwrap_or(false),
        is_configured: key.is_some_and(|value| !value.trim().is_empty()),
        key_suffix: key.and_then(key_suffix),
    }
}

/// Restricts ChatGPT connection IDs to bounded ASCII identifiers.
fn is_chatgpt_connection_id(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

/// Returns only a four-character suffix for identifying a configured key in settings UI.
fn key_suffix(value: &str) -> Option<String> {
    let suffix = value.chars().rev().take(4).collect::<String>();
    (suffix.chars().count() == 4).then(|| suffix.chars().rev().collect())
}

/// Removes storage implementation details from the provider-facing IPC error.
fn map_storage_error(_: McpStorageError) -> ProviderStorageError {
    ProviderStorageError::unavailable()
}

#[cfg(test)]
mod tests {
    use super::{
        http_provider_dialog_copy, requires_native_http_approval, update_custom_provider_values,
        BoundedText, CustomProviderConfig, SaveCustomProviderInput, MAXIMUM_KEY_BYTES,
        MAXIMUM_LOCALE_BYTES, MAXIMUM_PROVIDER_ID_BYTES, MAXIMUM_PROVIDER_NAME_BYTES,
        MAXIMUM_PROVIDER_PROTOCOL_BYTES, MAXIMUM_PROVIDER_URL_BYTES,
    };
    use crate::mcp_storage::SecretMap;
    use zeroize::Zeroizing;

    const PROVIDER_ID: &str = "custom_0123456789abcdef";
    const CONFIG_PREFIX: &str = "openchat.custom_provider.config.";
    const KEY_PREFIX: &str = "openchat.custom_provider.api_key.";

    fn provider_config(name: &str, base_url: &str, protocol: &str) -> CustomProviderConfig {
        super::validated_custom_provider(
            PROVIDER_ID.to_owned(),
            name.to_owned(),
            base_url.to_owned(),
            protocol.to_owned(),
            None,
        )
        .expect("test provider configuration should be valid")
    }

    fn saved_provider() -> (SecretMap, CustomProviderConfig) {
        let mut values = SecretMap::default();
        let config = provider_config("Provider", "https://provider.example/v1", "openAi");
        update_custom_provider_values(
            &mut values,
            config.clone(),
            Some(Zeroizing::new("old-secret-key".to_owned())),
        )
        .expect("test provider should save");
        (values, config)
    }

    fn assert_text_boundary<const MAXIMUM_BYTES: usize>() {
        let accepted = format!("\"{}\"", "x".repeat(MAXIMUM_BYTES));
        assert!(serde_json::from_str::<BoundedText<MAXIMUM_BYTES>>(&accepted).is_ok());

        let rejected = format!("\"{}\"", "x".repeat(MAXIMUM_BYTES + 1));
        assert!(serde_json::from_str::<BoundedText<MAXIMUM_BYTES>>(&rejected).is_err());
    }

    #[test]
    fn provider_request_text_accepts_each_boundary_and_rejects_the_next_byte() {
        assert_text_boundary::<MAXIMUM_KEY_BYTES>();
        assert_text_boundary::<MAXIMUM_PROVIDER_ID_BYTES>();
        assert_text_boundary::<MAXIMUM_PROVIDER_NAME_BYTES>();
        assert_text_boundary::<MAXIMUM_PROVIDER_PROTOCOL_BYTES>();
        assert_text_boundary::<MAXIMUM_PROVIDER_URL_BYTES>();
        assert_text_boundary::<MAXIMUM_LOCALE_BYTES>();
    }

    #[test]
    fn custom_provider_save_payload_uses_bounded_camel_case_fields() {
        let input = serde_json::from_str::<SaveCustomProviderInput>(
            r#"{"id":"custom_0123456789abcdef","name":"Provider","baseUrl":"https://provider.example/v1","protocol":"openAi","allowInsecureHttp":false,"apiKey":null,"locale":"tr-TR"}"#,
        )
        .expect("bounded provider payload should deserialize");
        assert_eq!(input.base_url.into_inner(), "https://provider.example/v1");
        assert_eq!(input.locale.into_inner(), "tr-TR");

        let oversized_locale = format!(
            r#"{{"id":"custom_0123456789abcdef","name":"Provider","baseUrl":"https://provider.example/v1","protocol":"openAi","allowInsecureHttp":false,"apiKey":null,"locale":"{}"}}"#,
            "x".repeat(MAXIMUM_LOCALE_BYTES + 1)
        );
        assert!(serde_json::from_str::<SaveCustomProviderInput>(&oversized_locale).is_err());
    }

    #[test]
    fn name_update_keeps_existing_key_when_destination_and_protocol_match() {
        let (mut values, _) = saved_provider();
        let summary = update_custom_provider_values(
            &mut values,
            provider_config("Renamed provider", "https://provider.example/v1/", "openAi"),
            None,
        )
        .expect("a name-only update should preserve the saved key");

        assert!(summary.is_configured);
        assert_eq!(
            values.get(&format!("{KEY_PREFIX}{PROVIDER_ID}")),
            Some("old-secret-key")
        );
    }

    #[test]
    fn endpoint_change_without_new_key_is_rejected_without_mutating_saved_values() {
        let (mut values, _) = saved_provider();
        let config_key = format!("{CONFIG_PREFIX}{PROVIDER_ID}");
        let key_key = format!("{KEY_PREFIX}{PROVIDER_ID}");
        let previous_config = values.get(&config_key).expect("saved config").to_owned();
        let previous_key = values.get(&key_key).expect("saved key").to_owned();

        let error = update_custom_provider_values(
            &mut values,
            provider_config("Provider", "https://attacker.example/v1", "openAi"),
            None,
        )
        .expect_err("changing the endpoint must require a new key");

        assert_eq!(
            error.code,
            "custom_provider_key_required_for_endpoint_change"
        );
        assert_eq!(values.get(&config_key), Some(previous_config.as_str()));
        assert_eq!(values.get(&key_key), Some(previous_key.as_str()));
    }

    #[test]
    fn protocol_change_without_new_key_is_rejected_without_mutating_saved_values() {
        let (mut values, _) = saved_provider();
        let config_key = format!("{CONFIG_PREFIX}{PROVIDER_ID}");
        let key_key = format!("{KEY_PREFIX}{PROVIDER_ID}");
        let previous_config = values.get(&config_key).expect("saved config").to_owned();
        let previous_key = values.get(&key_key).expect("saved key").to_owned();

        let error = update_custom_provider_values(
            &mut values,
            provider_config("Provider", "https://provider.example/v1", "anthropic"),
            None,
        )
        .expect_err("changing the API format must require a new key");

        assert_eq!(
            error.code,
            "custom_provider_key_required_for_endpoint_change"
        );
        assert_eq!(values.get(&config_key), Some(previous_config.as_str()));
        assert_eq!(values.get(&key_key), Some(previous_key.as_str()));
    }

    #[test]
    fn endpoint_change_with_replacement_key_updates_destination_and_credential() {
        let (mut values, _) = saved_provider();
        let summary = update_custom_provider_values(
            &mut values,
            provider_config("Provider", "https://new-provider.example/v1", "openAi"),
            Some(Zeroizing::new("replacement-secret".to_owned())),
        )
        .expect("a new key allows the endpoint change");

        assert!(summary.is_configured);
        assert_eq!(summary.base_url, "https://new-provider.example/v1");
        assert_eq!(
            values.get(&format!("{KEY_PREFIX}{PROVIDER_ID}")),
            Some("replacement-secret")
        );
    }

    #[test]
    fn new_provider_without_key_is_allowed() {
        let mut values = SecretMap::default();
        let summary = update_custom_provider_values(
            &mut values,
            provider_config("Provider", "https://provider.example/v1", "openAi"),
            None,
        )
        .expect("a new provider may not require authentication");

        assert!(!summary.is_configured);
        assert!(values.get(&format!("{KEY_PREFIX}{PROVIDER_ID}")).is_none());
    }

    #[test]
    fn new_http_provider_requires_explicit_approval() {
        let error = super::validated_custom_provider(
            PROVIDER_ID.to_owned(),
            "Provider".to_owned(),
            "http://127.0.0.1:8080/v1".to_owned(),
            "openAi".to_owned(),
            Some(false),
        )
        .expect_err("new HTTP providers require explicit approval");
        assert_eq!(error.code, "custom_provider_http_approval_required");

        let allowed = super::validated_custom_provider(
            PROVIDER_ID.to_owned(),
            "Provider".to_owned(),
            "http://127.0.0.1:8080/v1".to_owned(),
            "openAi".to_owned(),
            Some(true),
        )
        .expect("explicit HTTP approval should preserve local provider support");
        assert_eq!(allowed.allow_insecure_http, Some(true));
        assert!(requires_native_http_approval(&allowed));

        let secure = provider_config("Provider", "https://provider.example/v1", "openAi");
        assert!(!requires_native_http_approval(&secure));
    }

    #[test]
    fn http_confirmation_copy_uses_the_application_locale() {
        assert_eq!(http_provider_dialog_copy("tr-TR").allow, "HTTP'ye izin ver");
        assert_eq!(http_provider_dialog_copy("de-DE").cancel, "Abbrechen");
        assert_eq!(
            http_provider_dialog_copy("unknown").title,
            "OpenChat security confirmation"
        );
    }

    #[test]
    fn legacy_http_provider_summary_requires_explicit_approval() {
        let mut values = SecretMap::default();
        let legacy = provider_config("Provider", "http://127.0.0.1:8080/v1", "openAi");
        values.insert(
            format!("{CONFIG_PREFIX}{PROVIDER_ID}"),
            serde_json::to_string(&legacy).expect("legacy settings should serialize"),
        );

        let providers = super::load_custom_providers(&values)
            .expect("legacy HTTP provider settings remain visible for editing");
        assert_eq!(providers.len(), 1);
        assert!(!providers[0].allow_insecure_http);
    }

    #[test]
    fn orphaned_or_invalid_saved_configuration_requires_a_new_key() {
        let key_name = format!("{KEY_PREFIX}{PROVIDER_ID}");
        let config_name = format!("{CONFIG_PREFIX}{PROVIDER_ID}");
        let requested = provider_config("Provider", "https://provider.example/v1", "openAi");
        let mismatched_config = serde_json::to_string(&CustomProviderConfig {
            id: "custom_fedcba9876543210".to_owned(),
            name: "Provider".to_owned(),
            base_url: "https://provider.example/v1".to_owned(),
            protocol: "openAi".to_owned(),
            allow_insecure_http: None,
        })
        .expect("test configuration should serialize");

        for stored_config in [None, Some("not-json"), Some(mismatched_config.as_str())] {
            let mut values = SecretMap::default();
            values.insert(key_name.clone(), "old-secret-key".to_owned());
            if let Some(stored_config) = stored_config {
                values.insert(config_name.clone(), stored_config.to_owned());
            }

            let error = update_custom_provider_values(&mut values, requested.clone(), None)
                .expect_err("an unbound key must not be retained");

            assert_eq!(
                error.code,
                "custom_provider_key_required_for_endpoint_change"
            );
            assert_eq!(values.get(&key_name), Some("old-secret-key"));
            assert_eq!(values.get(&config_name), stored_config);
        }
    }
}
