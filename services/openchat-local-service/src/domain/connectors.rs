//! User-initiated retrieval and Ed25519 verification for connector metadata catalogs.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::signature::{ED25519, UnparsedPublicKey};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::Url;

const MAXIMUM_CATALOG_BYTES: usize = 64 * 1024;
const MAXIMUM_CATALOG_ENTRIES: usize = 50;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedCatalogEnvelope {
    payload: String,
    signature: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogPayload {
    version: u32,
    revision: u64,
    entries: Vec<Value>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
/// A signed connector manifest verified against a user-pinned public key.
pub(crate) struct VerifiedConnectorCatalog {
    pub(crate) source_url: String,
    pub(crate) revision: u64,
    pub(crate) key_fingerprint: String,
    pub(crate) payload_hash: String,
    pub(crate) entries: Vec<Value>,
}

/// Fetches a bounded HTTPS catalog and verifies its exact payload bytes against a pinned key.
///
/// The signature proves control of the supplied key only. It does not establish the publisher's
/// real-world identity, and the entries are returned as metadata for explicit user import.
pub(crate) async fn fetch_signed_catalog(
    source_url: &str,
    public_key_base64: &str,
) -> Result<VerifiedConnectorCatalog, ConnectorCatalogFetchError> {
    let source_url = parse_catalog_url(source_url)?;
    let public_key = URL_SAFE_NO_PAD
        .decode(public_key_base64)
        .map_err(|_| ConnectorCatalogFetchError::InvalidSource)?;
    if public_key.len() != 32 {
        return Err(ConnectorCatalogFetchError::InvalidSource);
    }

    let page = crate::tools::web_search::url_fetch::fetch_public_https_page(
        source_url,
        MAXIMUM_CATALOG_BYTES,
    )
    .await
    .map_err(|_| ConnectorCatalogFetchError::Unavailable)?;
    if page.truncated
        || !page
            .content_type
            .split(';')
            .next()
            .is_some_and(|content_type| content_type.trim() == "application/json")
    {
        return Err(ConnectorCatalogFetchError::InvalidCatalog);
    }
    let encoded_envelope =
        std::str::from_utf8(&page.bytes).map_err(|_| ConnectorCatalogFetchError::InvalidCatalog)?;
    let envelope: SignedCatalogEnvelope = serde_json::from_str(encoded_envelope)
        .map_err(|_| ConnectorCatalogFetchError::InvalidCatalog)?;
    if envelope.payload.len() > MAXIMUM_CATALOG_BYTES {
        return Err(ConnectorCatalogFetchError::InvalidCatalog);
    }
    let signature = URL_SAFE_NO_PAD
        .decode(envelope.signature)
        .map_err(|_| ConnectorCatalogFetchError::InvalidCatalog)?;
    if signature.len() != 64 {
        return Err(ConnectorCatalogFetchError::InvalidCatalog);
    }
    UnparsedPublicKey::new(&ED25519, &public_key)
        .verify(envelope.payload.as_bytes(), &signature)
        .map_err(|_| ConnectorCatalogFetchError::InvalidSignature)?;

    let payload: CatalogPayload = serde_json::from_str(&envelope.payload)
        .map_err(|_| ConnectorCatalogFetchError::InvalidCatalog)?;
    if payload.version != 1
        || payload.revision == 0
        || payload.entries.len() > MAXIMUM_CATALOG_ENTRIES
    {
        return Err(ConnectorCatalogFetchError::InvalidCatalog);
    }

    let fingerprint = Sha256::digest(&public_key);
    let payload_digest = Sha256::digest(envelope.payload.as_bytes());
    let key_fingerprint = fingerprint
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(VerifiedConnectorCatalog {
        source_url: page.url.to_string(),
        revision: payload.revision,
        key_fingerprint,
        payload_hash: payload_digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        entries: payload.entries,
    })
}

fn parse_catalog_url(value: &str) -> Result<Url, ConnectorCatalogFetchError> {
    if value.len() > 2048 || value.trim() != value {
        return Err(ConnectorCatalogFetchError::InvalidSource);
    }
    let url = Url::parse(value).map_err(|_| ConnectorCatalogFetchError::InvalidSource)?;
    if url.scheme() != "https:"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(ConnectorCatalogFetchError::InvalidSource);
    }
    crate::tools::web_search::url_fetch::validate_http_url(url)
        .map_err(|_| ConnectorCatalogFetchError::InvalidSource)
}

pub(crate) enum ConnectorCatalogFetchError {
    InvalidSource,
    InvalidCatalog,
    InvalidSignature,
    Unavailable,
}
