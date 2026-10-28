//! Resolves validated provider routes to zeroizing credentials and endpoint configuration.
//!
//! Saved secrets are read from the protected local store on demand; missing or malformed
//! configuration returns safe service errors without exposing stored values.

use std::str::FromStr;

use crate::secrets::{SecretMap, SecretStoreError};
use serde_json::Value;
use zeroize::Zeroizing;

use crate::{openai_compatible::CustomProviderConfig, protocol::ServiceError};

const CUSTOM_PROVIDER_CONFIG_PREFIX: &str = "openchat.custom_provider.config.";

/// Provides request-scoped access to credentials saved in the Windows user profile.
pub(crate) struct ProviderCredentials {
    values: SecretMap,
}

impl ProviderCredentials {
    /// Reads the existing DPAPI-protected credential map for the current Windows user.
    pub(crate) fn load(storage: &crate::storage::AppStorage) -> Result<Self, ServiceError> {
        let values = SecretMap::read_from_storage(storage).map_err(map_store_error)?;
        Ok(Self { values })
    }

    /// Returns a zeroizing copy of the key assigned to a validated provider route.
    pub(crate) fn api_key(
        &self,
        provider_id: &str,
        connection_id: Option<&str>,
    ) -> Result<Option<Zeroizing<String>>, ServiceError> {
        self.values
            .provider_api_key(provider_id, connection_id)
            .map_err(map_store_error)
    }

    /// Returns the API key assigned to a route or a safe missing-credential error.
    pub(crate) fn required_api_key(
        &self,
        provider_id: &str,
        connection_id: Option<&str>,
    ) -> Result<Zeroizing<String>, ServiceError> {
        self.api_key(provider_id, connection_id)?
            .ok_or_else(missing_credentials)
    }

    /// Loads and validates a custom-provider endpoint from the protected settings map.
    pub(crate) fn custom_provider_config(
        &self,
        provider_id: &str,
    ) -> Result<CustomProviderConfig, ServiceError> {
        if !crate::provider_ids::is_custom_provider_id(provider_id) {
            return Err(invalid_request_params());
        }
        let key = format!("{CUSTOM_PROVIDER_CONFIG_PREFIX}{provider_id}");
        let config = self.values.get(&key).ok_or_else(missing_credentials)?;
        let value = Value::from_str(config).map_err(|_| storage_unavailable())?;
        let config = CustomProviderConfig::from_value(&value)?;
        if config.id != provider_id {
            return Err(storage_unavailable());
        }
        Ok(config)
    }
}

/// Returns whether a route reads an API key from the shared protected settings map.
pub(crate) fn uses_saved_api_key(provider_id: &str) -> bool {
    provider_id == "opencode"
        || provider_id == "chatgpt_api"
        || crate::provider_ids::is_custom_provider_id(provider_id)
        || crate::openai_compatible::api_compatible_provider(provider_id).is_some()
}

/// Returns whether a route cannot run without a saved provider API key.
pub(crate) fn requires_saved_api_key(provider_id: &str) -> bool {
    provider_id == "chatgpt_api"
        || (provider_id != "opencode"
            && provider_id != "kilo"
            && !crate::provider_ids::is_custom_provider_id(provider_id)
            && crate::openai_compatible::api_compatible_provider(provider_id).is_some())
}

/// Returns the public error used when a configured provider key is no longer available.
pub(crate) fn missing_credentials() -> ServiceError {
    ServiceError::new(
        "provider_credentials_missing",
        "The saved credentials for this provider are unavailable. Check the provider settings.",
        false,
    )
}

fn map_store_error(error: SecretStoreError) -> ServiceError {
    match error {
        SecretStoreError::Unavailable => storage_unavailable(),
        SecretStoreError::InvalidProviderRoute => invalid_request_params(),
    }
}

fn storage_unavailable() -> ServiceError {
    ServiceError::new(
        "provider_credential_storage_unavailable",
        "Saved provider credentials could not be accessed.",
        false,
    )
}

fn invalid_request_params() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "A required local service parameter is missing or invalid.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::ProviderCredentials;
    use crate::secrets::SecretMap;

    #[test]
    fn provider_keys_are_bound_to_the_saved_route_identifier() {
        let mut values = SecretMap::default();
        values.insert(
            "openchat.chatgpt.api_key.primary_account".to_owned(),
            "test-provider-key".to_owned(),
        );
        let credentials = ProviderCredentials { values };

        assert_eq!(
            credentials
                .api_key("chatgpt_api", Some("primary_account"))
                .expect("saved account route")
                .as_deref()
                .map(String::as_str),
            Some("test-provider-key")
        );
        assert_eq!(
            credentials
                .api_key("chatgpt_api", Some("other_account"))
                .expect("unconfigured account route"),
            None
        );
    }

    #[test]
    fn custom_provider_configuration_comes_from_saved_settings() {
        let provider_id = "custom_provider_123456";
        let mut values = SecretMap::default();
        values.insert(
            format!("openchat.custom_provider.config.{provider_id}"),
            format!(
                r#"{{"id":"{provider_id}","name":"Saved provider","baseUrl":"https://api.example.test/v1","protocol":"openAi"}}"#
            ),
        );
        let credentials = ProviderCredentials { values };

        let config = credentials
            .custom_provider_config(provider_id)
            .expect("saved custom provider route");
        assert_eq!(config.id, provider_id);
        assert_eq!(config.base_url, "https://api.example.test/v1");
    }

    #[test]
    fn custom_provider_http_requires_explicit_approval_for_existing_and_new_settings() {
        let provider_id = "custom_provider_123456";
        let rejected = serde_json::json!({
            "id": provider_id,
            "baseUrl": "http://127.0.0.1:8080/v1",
            "protocol": "openAi",
            "allowInsecureHttp": false,
        });
        let mut values = SecretMap::default();
        values.insert(
            format!("openchat.custom_provider.config.{provider_id}"),
            rejected.to_string(),
        );
        let credentials = ProviderCredentials { values };
        assert_eq!(
            credentials
                .custom_provider_config(provider_id)
                .expect_err("an explicitly unapproved HTTP route must be rejected")
                .code,
            "custom_provider_http_approval_required"
        );

        let approved = serde_json::json!({
            "id": provider_id,
            "baseUrl": "http://127.0.0.1:8080/v1",
            "protocol": "openAi",
            "allowInsecureHttp": true,
            "nativeHttpConfirmed": true,
        });
        let config = super::CustomProviderConfig::from_value(&approved)
            .expect("native-confirmed HTTP approval should retain local provider access");
        assert_eq!(config.base_url, "http://127.0.0.1:8080/v1");

        for base_url in [
            "http://localhost:8080/v1",
            "http://[::1]:8080/v1",
            "http://127.255.255.255:8080/v1",
            "http://127.1:8080/v1",
            "http://2130706433:8080/v1",
            "http://0x7f000001:8080/v1",
            "http://0177.0.0.1:8080/v1",
        ] {
            let local = serde_json::json!({
                "id": provider_id,
                "baseUrl": base_url,
                "protocol": "openAi",
                "allowInsecureHttp": true,
                "nativeHttpConfirmed": true,
            });
            assert!(
                super::CustomProviderConfig::from_value(&local).is_ok(),
                "loopback provider {base_url} should remain supported"
            );
        }

        for base_url in [
            "http://provider.example/v1",
            "http://192.168.1.12:8080/v1",
            "http://127.0.0.1.provider.example/v1",
            "http://localhost./v1",
            "http://0.0.0.0:8080/v1",
            "http://[::]:8080/v1",
            "http://[::ffff:127.0.0.1]:8080/v1",
        ] {
            let remote = serde_json::json!({
                "id": provider_id,
                "baseUrl": base_url,
                "protocol": "openAi",
                "allowInsecureHttp": true,
                "nativeHttpConfirmed": true,
            });
            assert_eq!(
                super::CustomProviderConfig::from_value(&remote)
                    .expect_err("non-loopback HTTP must fail before any provider request")
                    .code,
                "custom_provider_http_loopback_required",
                "unexpected error for {base_url}"
            );
        }

        let secure_remote = serde_json::json!({
            "id": provider_id,
            "baseUrl": "https://provider.example/v1",
            "protocol": "openAi",
            "allowInsecureHttp": false,
            "nativeHttpConfirmed": false,
        });
        assert!(super::CustomProviderConfig::from_value(&secure_remote).is_ok());

        let legacy_approval = serde_json::json!({
            "id": provider_id,
            "baseUrl": "http://127.0.0.1:8080/v1",
            "protocol": "openAi",
            "allowInsecureHttp": true,
        });
        assert_eq!(
            super::CustomProviderConfig::from_value(&legacy_approval)
                .expect_err("legacy approval has no native confirmation provenance")
                .code,
            "custom_provider_http_approval_required"
        );

        let legacy = serde_json::json!({
            "id": provider_id,
            "baseUrl": "http://127.0.0.1:8080/v1",
            "protocol": "openAi",
        });
        assert_eq!(
            super::CustomProviderConfig::from_value(&legacy)
                .expect_err("a saved HTTP route without approval must be rejected")
                .code,
            "custom_provider_http_approval_required"
        );
    }

    #[test]
    fn missing_required_provider_key_has_a_stable_error_code() {
        let credentials = ProviderCredentials {
            values: SecretMap::default(),
        };

        assert_eq!(
            credentials
                .required_api_key("gemini", Some("gemini"))
                .expect_err("missing provider key")
                .code,
            "provider_credentials_missing"
        );
    }
}
