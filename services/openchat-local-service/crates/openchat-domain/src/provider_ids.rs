//! Canonical provider identifier validation shared with the desktop host.

/// Returns the supported built-in API-compatible provider identifiers.
pub fn compatible_provider_ids() -> impl Iterator<Item = &'static str> {
    crate::openai_compatible::api_compatible_provider_ids()
}

/// Returns whether an identifier names a supported built-in API-compatible provider.
pub fn is_compatible_provider_id(provider_id: &str) -> bool {
    crate::openai_compatible::is_api_compatible_provider(provider_id)
}

/// Returns whether an identifier has the canonical application-generated custom-provider form.
pub fn is_custom_provider_id(provider_id: &str) -> bool {
    (19..=64).contains(&provider_id.len())
        && provider_id.starts_with("custom_")
        && provider_id[7..]
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::{compatible_provider_ids, is_compatible_provider_id, is_custom_provider_id};
    use std::collections::HashSet;

    #[test]
    fn compatible_provider_catalog_is_unique_and_used_for_validation() {
        let provider_ids = compatible_provider_ids().collect::<Vec<_>>();

        assert_eq!(provider_ids.len(), 16);
        assert_eq!(
            provider_ids.iter().copied().collect::<HashSet<_>>().len(),
            provider_ids.len()
        );
        assert!(provider_ids.iter().all(|id| is_compatible_provider_id(id)));
        assert!(is_compatible_provider_id("anthropic"));
        assert!(!is_compatible_provider_id("not-a-provider"));
    }

    #[test]
    fn custom_provider_id_uses_the_shared_length_and_character_rule() {
        assert!(is_custom_provider_id("custom_provider_123456"));
        assert!(is_custom_provider_id(&format!("custom_{}", "a".repeat(57))));
        assert!(!is_custom_provider_id(&format!(
            "custom_{}",
            "a".repeat(58)
        )));
        assert!(!is_custom_provider_id("custom_UPPERCASE123"));
    }
}
