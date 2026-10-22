//! RPC entry points for user-managed connector metadata sources.

use serde_json::Value;

use crate::{
    connector_packages::{self, InstalledPackageReference, PackageError},
    connectors,
    protocol::ServiceError,
    storage::AppStorage,
    tools,
};

use super::{invalid_request_params, required_string};

/// Fetches a signed catalog only after an explicit renderer request.
pub(super) async fn dispatch(
    method: &str,
    params: &Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    match method {
        "connector.catalog.fetch_signed" => {
            let source_url = required_string(params, "sourceUrl")?;
            let public_key = required_string(params, "publicKeyBase64")?;
            if source_url.len() > 2048 || public_key.len() > 64 {
                return Err(invalid_request_params());
            }
            let catalog = connectors::fetch_signed_catalog(source_url, public_key)
                .await
                .map_err(map_catalog_error)?;
            serde_json::to_value(catalog).map_err(|_| {
                ServiceError::new(
                    "connector_catalog_unavailable",
                    "The connector catalog could not be loaded.",
                    true,
                )
            })
        }
        "connector.package.install_signed" => {
            let project_id = required_string(params, "projectId")?;
            let source_url = required_string(params, "sourceUrl")?;
            let public_key = required_string(params, "publicKeyBase64")?;
            let id = required_string(params, "id")?;
            let version = required_string(params, "version")?;
            if source_url.len() > 2048
                || public_key.len() != 43
                || id.len() > 24
                || version.len() > 64
            {
                return Err(invalid_request_params());
            }
            let project_root = super::projects::project_root_for_mcp(storage, project_id)?;
            let package = connector_packages::install_signed_package(
                storage.root(),
                &project_root,
                source_url,
                public_key,
                id,
                version,
            )
            .await
            .map_err(map_package_error)?;
            serde_json::to_value(package).map_err(|_| {
                ServiceError::new(
                    "connector_package_unavailable",
                    "The connector package could not be installed.",
                    false,
                )
            })
        }
        "connector.package.remove" => {
            let project_id = required_string(params, "projectId")?;
            let id = required_string(params, "id")?;
            let package = params
                .get("package")
                .and_then(|value| {
                    serde_json::from_value::<InstalledPackageReference>(value.clone()).ok()
                })
                .ok_or_else(invalid_request_params)?;
            let project_root = super::projects::project_root_for_mcp(storage, project_id)?;
            if tools::mcp::package_is_referenced(&project_root, id, &package, storage.root())? {
                return Err(ServiceError::new(
                    "connector_package_in_use",
                    "The connector package is still referenced by the project configuration.",
                    false,
                ));
            }
            connector_packages::remove_installed_package(
                storage.root(),
                &project_root,
                id,
                &package,
            )
            .map_err(map_package_error)?;
            Ok(serde_json::json!({"removed": true}))
        }
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

fn map_package_error(error: PackageError) -> ServiceError {
    match error {
        PackageError::InvalidCatalog => ServiceError::new(
            "connector_package_catalog_invalid",
            "The signed connector package metadata is invalid.",
            false,
        ),
        PackageError::PackageUnavailable => ServiceError::new(
            "connector_package_unavailable",
            "The signed catalog does not contain this package version.",
            false,
        ),
        PackageError::InvalidPackage => ServiceError::new(
            "connector_package_integrity_failed",
            "The connector package failed its integrity or executable-format check.",
            false,
        ),
        PackageError::InstallConflict => ServiceError::new(
            "connector_package_version_conflict",
            "A different package already uses this connector version.",
            false,
        ),
        PackageError::Unavailable => ServiceError::new(
            "connector_package_unavailable",
            "The connector package could not be installed or removed.",
            true,
        ),
    }
}

fn map_catalog_error(error: connectors::ConnectorCatalogFetchError) -> ServiceError {
    match error {
        connectors::ConnectorCatalogFetchError::InvalidSource => ServiceError::new(
            "connector_catalog_invalid_source",
            "The connector catalog source is invalid.",
            false,
        ),
        connectors::ConnectorCatalogFetchError::InvalidCatalog => ServiceError::new(
            "connector_catalog_invalid_data",
            "The connector catalog returned invalid data.",
            false,
        ),
        connectors::ConnectorCatalogFetchError::InvalidSignature => ServiceError::new(
            "connector_catalog_signature_invalid",
            "The connector catalog signature does not match the pinned key.",
            false,
        ),
        connectors::ConnectorCatalogFetchError::Unavailable => ServiceError::new(
            "connector_catalog_unavailable",
            "The connector catalog could not be reached.",
            true,
        ),
    }
}
