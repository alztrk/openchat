//! Shared HTTP header construction for credentials sent to remote services.

use reqwest::header::{HeaderValue, InvalidHeaderValue};

/// Creates a header value that Reqwest redacts from request debug output.
///
/// Invalid header bytes are returned to the caller so each boundary can expose its own safe error.
pub(crate) fn sensitive_api_key_header(api_key: &str) -> Result<HeaderValue, InvalidHeaderValue> {
    let mut header = HeaderValue::from_bytes(api_key.as_bytes())?;
    header.set_sensitive(true);
    Ok(header)
}

#[cfg(test)]
mod tests {
    use super::sensitive_api_key_header;

    #[test]
    fn api_key_header_is_sensitive_and_rejects_invalid_values() {
        let header = sensitive_api_key_header("test-key").expect("valid API key header");
        assert!(header.is_sensitive());
        assert!(!format!("{header:?}").contains("test-key"));
        assert!(sensitive_api_key_header("invalid\nkey").is_err());
    }
}
