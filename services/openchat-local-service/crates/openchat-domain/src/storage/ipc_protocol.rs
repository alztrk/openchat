//! Bounded wire types shared by the desktop storage client and Windows broker.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use rusqlite::{Connection, Error, Result, ffi, params_from_iter, types::Value};
use serde::{Deserialize, Serialize};

pub(crate) const STORAGE_PROTOCOL_VERSION: u32 = 3;
pub(crate) const MAX_STORAGE_FRAME_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_STORAGE_SQL_BYTES: usize = 256 * 1024;
pub(crate) const MAX_STORAGE_PARAMETERS: usize = 2_000;
pub(crate) const MAX_STORAGE_ROWS: usize = 100_000;
pub(crate) const MAX_STORAGE_COLUMNS: usize = 512;
pub(crate) const MAX_STORAGE_TRANSFER_CHUNK_BYTES: usize = 256 * 1024;
pub(crate) const MAX_STORAGE_IMPORT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub(crate) const MAX_STORAGE_SNAPSHOT_BYTES: u64 = 32 * 1024 * 1024 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub(crate) enum StorageRequest {
    Hello {
        version: u32,
    },
    Execute {
        sql: String,
        parameters: Vec<StorageValue>,
    },
    Query {
        sql: String,
        parameters: Vec<StorageValue>,
    },
    ExecuteBatch {
        sql: String,
    },
    Begin {
        behavior: TransactionBehavior,
    },
    Commit,
    Rollback,
    LastInsertRowId,
    Changes,
    VaultReadMap,
    VaultWriteMap {
        secrets: crate::secrets::SecretMap,
    },
    VaultGet {
        key: String,
    },
    VaultSet {
        secrets: crate::secrets::SecretMap,
    },
    VaultDelete {
        key: String,
    },
    VaultHas {
        key: String,
    },
    BeginSnapshot,
    ReadSnapshotChunk {
        snapshot_id: String,
        offset: u64,
    },
    EndSnapshot {
        snapshot_id: String,
    },
    LegacyImportStatus,
    CompleteEmptyLegacyImport {
        secrets: crate::secrets::SecretMap,
        preserve_legacy_source: bool,
    },
    BeginLegacyImport {
        total_bytes: u64,
        sha256: String,
    },
    WriteLegacyImportChunk {
        import_id: String,
        offset: u64,
        data: String,
    },
    CommitLegacyImport {
        import_id: String,
        secrets: crate::secrets::SecretMap,
    },
    CommitPendingProfileRestore,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub(crate) enum StorageResponse {
    Ready {
        version: u32,
    },
    Executed {
        changed_rows: usize,
        last_insert_row_id: i64,
    },
    Rows {
        columns: Vec<String>,
        rows: Vec<Vec<StorageValue>>,
    },
    Integer {
        value: i64,
    },
    Changes {
        value: u64,
    },
    Unit,
    VaultMap {
        secrets: crate::secrets::SecretMap,
    },
    VaultUpdated,
    VaultContains {
        exists: bool,
    },
    SnapshotStarted {
        snapshot_id: String,
        total_bytes: u64,
        sha256: String,
    },
    SnapshotChunk {
        offset: u64,
        data: String,
        complete: bool,
    },
    SnapshotEnded,
    LegacyImportStatus {
        status: LegacyImportStatus,
    },
    LegacyImportStarted {
        import_id: String,
    },
    LegacyImportChunkWritten {
        next_offset: u64,
    },
    LegacyImportCompleted,
    Error {
        code: i32,
        message: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LegacyImportStatus {
    PendingEmpty,
    Completed,
    CompletedWithLegacySourcePreserved,
    ExistingData,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TransactionBehavior {
    Deferred,
    Immediate,
    Exclusive,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub(crate) enum StorageValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(String),
}

pub(crate) fn handle_request(
    connection: &mut Connection,
    request: StorageRequest,
) -> StorageResponse {
    match handle_request_inner(connection, request) {
        Ok(response) => response,
        Err(error) => storage_error_response(error),
    }
}

pub(crate) fn storage_error_response(error: Error) -> StorageResponse {
    let code = error
        .sqlite_error_code()
        .map_or(ffi::SQLITE_ERROR, |code| code as i32);
    StorageResponse::Error {
        code,
        message: safe_error_message(code).to_owned(),
    }
}

fn handle_request_inner(
    connection: &mut Connection,
    request: StorageRequest,
) -> Result<StorageResponse> {
    match request {
        StorageRequest::Hello { version } if version == STORAGE_PROTOCOL_VERSION => {
            Ok(StorageResponse::Ready {
                version: STORAGE_PROTOCOL_VERSION,
            })
        }
        StorageRequest::Hello { .. } => Err(protocol_error("unsupported storage protocol version")),
        StorageRequest::Execute { sql, parameters } => {
            validate_sql_and_parameters(&sql, &parameters)?;
            let values = storage_values_into_sql(parameters)?;
            let changed_rows = connection.execute(&sql, params_from_iter(values.iter()))?;
            Ok(StorageResponse::Executed {
                changed_rows,
                last_insert_row_id: connection.last_insert_rowid(),
            })
        }
        StorageRequest::Query { sql, parameters } => {
            validate_sql_and_parameters(&sql, &parameters)?;
            let values = storage_values_into_sql(parameters)?;
            let mut statement = connection.prepare(&sql)?;
            let column_count = statement.column_count();
            if column_count > MAX_STORAGE_COLUMNS {
                return Err(protocol_error("storage query returned too many columns"));
            }
            let columns = statement
                .column_names()
                .into_iter()
                .map(str::to_owned)
                .collect();
            let mut cursor = statement.query(params_from_iter(values.iter()))?;
            let mut rows = Vec::new();
            while let Some(row) = cursor.next()? {
                if rows.len() >= MAX_STORAGE_ROWS {
                    return Err(protocol_error("storage query returned too many rows"));
                }
                let mut values = Vec::with_capacity(column_count);
                for column_index in 0..column_count {
                    let value = row.get::<_, Value>(column_index)?;
                    values.push(StorageValue::try_from(value).map_err(|_| {
                        protocol_error("storage query returned an unsupported value")
                    })?);
                }
                rows.push(values);
            }
            Ok(StorageResponse::Rows { columns, rows })
        }
        StorageRequest::ExecuteBatch { sql } => {
            validate_sql(&sql)?;
            connection.execute_batch(&sql)?;
            Ok(StorageResponse::Unit)
        }
        StorageRequest::Begin { behavior } => {
            let statement = match behavior {
                TransactionBehavior::Deferred => "BEGIN DEFERRED",
                TransactionBehavior::Immediate => "BEGIN IMMEDIATE",
                TransactionBehavior::Exclusive => "BEGIN EXCLUSIVE",
            };
            connection.execute_batch(statement)?;
            Ok(StorageResponse::Unit)
        }
        StorageRequest::Commit => {
            connection.execute_batch("COMMIT")?;
            Ok(StorageResponse::Unit)
        }
        StorageRequest::Rollback => {
            connection.execute_batch("ROLLBACK")?;
            Ok(StorageResponse::Unit)
        }
        StorageRequest::LastInsertRowId => Ok(StorageResponse::Integer {
            value: connection.last_insert_rowid(),
        }),
        StorageRequest::Changes => Ok(StorageResponse::Changes {
            value: connection.changes(),
        }),
        StorageRequest::VaultReadMap
        | StorageRequest::VaultWriteMap { .. }
        | StorageRequest::VaultGet { .. }
        | StorageRequest::VaultSet { .. }
        | StorageRequest::VaultDelete { .. }
        | StorageRequest::VaultHas { .. }
        | StorageRequest::BeginSnapshot
        | StorageRequest::ReadSnapshotChunk { .. }
        | StorageRequest::EndSnapshot { .. }
        | StorageRequest::LegacyImportStatus
        | StorageRequest::CompleteEmptyLegacyImport { .. }
        | StorageRequest::BeginLegacyImport { .. }
        | StorageRequest::WriteLegacyImportChunk { .. }
        | StorageRequest::CommitLegacyImport { .. }
        | StorageRequest::CommitPendingProfileRestore => Err(protocol_error(
            "profile operations must use their dedicated storage handler",
        )),
    }
}

fn validate_sql_and_parameters(sql: &str, parameters: &[StorageValue]) -> Result<()> {
    validate_sql(sql)?;
    if parameters.len() > MAX_STORAGE_PARAMETERS {
        return Err(protocol_error("storage request has too many parameters"));
    }
    Ok(())
}

fn validate_sql(sql: &str) -> Result<()> {
    if sql.is_empty() || sql.len() > MAX_STORAGE_SQL_BYTES {
        return Err(protocol_error(
            "storage SQL is empty or exceeds its size limit",
        ));
    }
    let keywords = sql_keywords(sql);
    if keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "ATTACH" | "DETACH" | "LOAD_EXTENSION"))
        || keywords.windows(2).any(|pair| pair == ["VACUUM", "INTO"])
        || (keywords.first().is_some_and(|keyword| keyword == "PRAGMA")
            && keywords.iter().any(|keyword| {
                matches!(
                    keyword.as_str(),
                    "WRITABLE_SCHEMA" | "TEMP_STORE_DIRECTORY" | "DATA_STORE_DIRECTORY"
                )
            }))
    {
        return Err(protocol_error(
            "storage SQL requested a restricted SQLite operation",
        ));
    }
    Ok(())
}

fn sql_keywords(sql: &str) -> Vec<String> {
    let bytes = sql.as_bytes();
    let mut keywords = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' | b'"' | b'`' => {
                let quote = bytes[index];
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == quote {
                        index += 1;
                        if bytes.get(index) == Some(&quote) {
                            index += 1;
                            continue;
                        }
                        break;
                    }
                    index += 1;
                }
            }
            b'[' => {
                index += 1;
                while index < bytes.len() && bytes[index] != b']' {
                    index += 1;
                }
                index = index.saturating_add(1);
            }
            b'-' if bytes.get(index + 1) == Some(&b'-') => {
                index += 2;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index += 2;
                while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/')
                {
                    index += 1;
                }
                index = (index + 2).min(bytes.len());
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = index;
                index += 1;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
                {
                    index += 1;
                }
                keywords.push(sql[start..index].to_ascii_uppercase());
            }
            _ => index += 1,
        }
    }
    keywords
}

fn safe_error_message(code: i32) -> &'static str {
    match code {
        ffi::SQLITE_BUSY | ffi::SQLITE_LOCKED => "The database is busy.",
        ffi::SQLITE_CORRUPT | ffi::SQLITE_NOTADB => "The database failed an integrity check.",
        ffi::SQLITE_CANTOPEN | ffi::SQLITE_PERM | ffi::SQLITE_IOERR => {
            "The protected database is unavailable."
        }
        ffi::SQLITE_FULL => "The database storage is full.",
        ffi::SQLITE_TOOBIG => "The database request exceeded its size limit.",
        ffi::SQLITE_MISUSE => "The database request is invalid.",
        _ => "The database operation failed.",
    }
}

fn storage_values_into_sql(values: Vec<StorageValue>) -> Result<Vec<Value>> {
    values
        .into_iter()
        .map(|value| {
            Value::try_from(value)
                .map_err(|_| protocol_error("storage request contains an invalid value"))
        })
        .collect()
}

fn protocol_error(message: &str) -> Error {
    Error::SqliteFailure(
        ffi::Error::new(ffi::SQLITE_MISUSE),
        Some(message.to_owned()),
    )
}

impl TryFrom<Value> for StorageValue {
    type Error = String;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Null => Ok(Self::Null),
            Value::Integer(value) => Ok(Self::Integer(value)),
            Value::Real(value) if value.is_finite() => Ok(Self::Real(value)),
            Value::Real(_) => Err("non-finite SQLite values cannot cross the storage pipe".into()),
            Value::Text(value) => Ok(Self::Text(value)),
            Value::Blob(value) => Ok(Self::Blob(STANDARD.encode(value))),
        }
    }
}

impl TryFrom<StorageValue> for Value {
    type Error = String;

    fn try_from(value: StorageValue) -> Result<Self, Self::Error> {
        match value {
            StorageValue::Null => Ok(Self::Null),
            StorageValue::Integer(value) => Ok(Self::Integer(value)),
            StorageValue::Real(value) if value.is_finite() => Ok(Self::Real(value)),
            StorageValue::Real(_) => {
                Err("non-finite SQLite values cannot cross the storage pipe".into())
            }
            StorageValue::Text(value) => Ok(Self::Text(value)),
            StorageValue::Blob(value) => STANDARD
                .decode(value)
                .map(Self::Blob)
                .map_err(|_| "invalid binary value in storage pipe frame".into()),
        }
    }
}
