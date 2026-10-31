//! Host-only secret-map operations used by the native desktop process.

use serde_json::{Value, json};
use zeroize::Zeroize;

use crate::{protocol::ServiceError, secrets::SecretMap, storage::AppStorage};

const MAXIMUM_SECRET_MAP_BYTES: usize = 16 * 1024 * 1024;
const MAXIMUM_SECRET_MAP_ENTRIES: usize = 16_384;

pub(super) fn dispatch(
    method: &str,
    params: &mut Value,
    storage: &AppStorage,
) -> Result<Value, ServiceError> {
    match method {
        "internal.secrets.map.read" => {
            if params.as_object().is_none_or(|params| !params.is_empty()) {
                return Err(invalid_request());
            }
            let secrets = SecretMap::read_from_storage(storage).map_err(storage_error)?;
            serde_json::to_value(secrets)
                .map(|secrets| json!({"secrets": secrets}))
                .map_err(|_| storage_error(()))
        }
        "internal.secrets.map.write" => {
            let params = params.as_object_mut().ok_or_else(invalid_request)?;
            if params.len() != 1 {
                return Err(invalid_request());
            }
            let mut serialized = params.remove("secrets").ok_or_else(invalid_request)?;
            if let Err(error) = validate_serialized_map(&serialized) {
                zeroize_json_value(&mut serialized);
                return Err(error);
            }
            let secrets =
                serde_json::from_value::<SecretMap>(serialized).map_err(|_| invalid_request())?;
            secrets.write_to_storage(storage).map_err(storage_error)?;
            Ok(json!({"saved": true}))
        }
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

fn zeroize_json_value(value: &mut Value) {
    match value {
        Value::String(text) => text.zeroize(),
        Value::Array(values) => {
            for value in values {
                zeroize_json_value(value);
            }
        }
        Value::Object(fields) => {
            for value in fields.values_mut() {
                zeroize_json_value(value);
            }
        }
        _ => {}
    }
}

fn validate_serialized_map(value: &Value) -> Result<(), ServiceError> {
    let Some(secrets) = value.as_object() else {
        return Err(invalid_request());
    };
    if secrets.len() > MAXIMUM_SECRET_MAP_ENTRIES
        || secrets
            .iter()
            .try_fold(0usize, |total, (key, value)| {
                total
                    .checked_add(key.len())
                    .and_then(|total| total.checked_add(value.as_str()?.len()))
                    .filter(|total| *total <= MAXIMUM_SECRET_MAP_BYTES)
            })
            .is_none()
    {
        return Err(invalid_request());
    }
    Ok(())
}

fn invalid_request() -> ServiceError {
    ServiceError::new(
        "invalid_request_params",
        "The local service request is invalid.",
        false,
    )
}

fn storage_error(_: impl std::fmt::Debug) -> ServiceError {
    ServiceError::new(
        "provider_credential_storage_unavailable",
        "Saved provider credentials could not be accessed.",
        false,
    )
}
