//! Backend-neutral access to the primary SQLite storage.
//!
//! The desktop process can use the local FIFO queue while a Windows build can route the same
//! operations through the protected storage broker.  Queries are materialized into owned rows at
//! this boundary.  That keeps callbacks independent of a local SQLite statement lifetime and of
//! the broker's framed response buffer.

use std::path::Path;
use std::{marker::PhantomData, sync::Arc};

use rusqlite::{
    CachedStatement, Error, Params, Result, Statement, Transaction, TransactionBehavior, ffi,
    types::{FromSql, FromSqlError, Value, ValueRef},
};

#[cfg(test)]
use rusqlite::Connection;

use super::{
    database_key::DatabaseKey,
    database_queue::QueuedConnection,
    ipc_protocol::{MAX_STORAGE_COLUMNS, MAX_STORAGE_ROWS},
};

#[cfg(windows)]
use super::broker_connection::{BrokerConnection, BrokerRows, BrokerStatement, BrokerTransaction};

/// A connection backed by the local operation queue or, on Windows, by the storage broker.
pub(crate) struct StorageConnection<'a> {
    backend: StorageBackend<'a>,
    database_key: Option<&'a DatabaseKey>,
}

enum StorageBackend<'a> {
    Local(QueuedConnection<'a>),
    #[cfg(test)]
    Standalone(Connection),
    #[cfg(windows)]
    Remote(BrokerConnection),
}

impl<'a> StorageConnection<'a> {
    /// Wraps a connection lease acquired from the local FIFO queue.
    pub(crate) fn local(connection: QueuedConnection<'a>, database_key: &'a DatabaseKey) -> Self {
        Self {
            backend: StorageBackend::Local(connection),
            database_key: Some(database_key),
        }
    }

    #[cfg(test)]
    pub(crate) fn standalone(connection: Connection) -> Self {
        Self {
            backend: StorageBackend::Standalone(connection),
            database_key: None,
        }
    }

    /// Wraps an already connected Windows storage broker client.
    #[cfg(windows)]
    pub(crate) fn remote(connection: BrokerConnection) -> Self {
        Self {
            backend: StorageBackend::Remote(connection),
            database_key: None,
        }
    }

    /// Executes one parameterized statement.
    pub(crate) fn execute<P: Params>(&self, sql: &str, params: P) -> Result<usize> {
        match &self.backend {
            StorageBackend::Local(connection) => connection.execute(sql, params),
            #[cfg(test)]
            StorageBackend::Standalone(connection) => connection.execute(sql, params),
            #[cfg(windows)]
            StorageBackend::Remote(connection) => connection.execute(sql, params),
        }
    }

    /// Executes a batch of SQL statements.
    pub(crate) fn execute_batch(&self, sql: &str) -> Result<()> {
        match &self.backend {
            StorageBackend::Local(connection) => connection.execute_batch(sql),
            #[cfg(test)]
            StorageBackend::Standalone(connection) => connection.execute_batch(sql),
            #[cfg(windows)]
            StorageBackend::Remote(connection) => connection.execute_batch(sql),
        }
    }

    /// Runs a query and returns its owned rows.
    pub(crate) fn query<P: Params>(&self, sql: &str, params: P) -> Result<StorageRows> {
        let mut statement = self.prepare(sql)?;
        statement.query(params)
    }

    /// Runs a query expected to return at least one row.
    pub(crate) fn query_row<T, P, F>(&self, sql: &str, params: P, f: F) -> Result<T>
    where
        P: Params,
        F: FnOnce(&StorageRow) -> Result<T>,
    {
        let mut statement = self.prepare(sql)?;
        statement.query_row(params, f)
    }

    /// Runs a query and maps its owned rows.
    pub(crate) fn query_map<T, P, F>(
        &self,
        sql: &str,
        params: P,
        f: F,
    ) -> Result<StorageMappedRows<T, F>>
    where
        P: Params,
        F: FnMut(&StorageRow) -> Result<T>,
    {
        let mut statement = self.prepare(sql)?;
        statement.query_map(params, f)
    }

    /// Prepares a statement.  Local statements are retained only for the duration of the returned
    /// wrapper; query results themselves are copied into owned rows.
    pub(crate) fn prepare(&self, sql: &str) -> Result<StorageStatement<'_>> {
        match &self.backend {
            StorageBackend::Local(connection) => Ok(StorageStatement {
                backend: StorageStatementBackend::Local(connection.prepare(sql)?),
            }),
            #[cfg(test)]
            StorageBackend::Standalone(connection) => Ok(StorageStatement {
                backend: StorageStatementBackend::Local(connection.prepare(sql)?),
            }),
            #[cfg(windows)]
            StorageBackend::Remote(connection) => Ok(StorageStatement {
                backend: StorageStatementBackend::Remote(connection.prepare(sql)?),
            }),
        }
    }

    /// Begins a deferred transaction.
    pub(crate) fn transaction(&mut self) -> Result<StorageTransaction<'_>> {
        self.transaction_with_behavior(TransactionBehavior::Deferred)
    }

    /// Begins a transaction with the requested SQLite behavior.
    pub(crate) fn transaction_with_behavior(
        &mut self,
        behavior: TransactionBehavior,
    ) -> Result<StorageTransaction<'_>> {
        match &mut self.backend {
            StorageBackend::Local(connection) => Ok(StorageTransaction {
                backend: StorageTransactionBackend::Local(
                    connection.transaction_with_behavior(behavior)?,
                ),
            }),
            #[cfg(test)]
            StorageBackend::Standalone(connection) => Ok(StorageTransaction {
                backend: StorageTransactionBackend::Local(
                    connection.transaction_with_behavior(behavior)?,
                ),
            }),
            #[cfg(windows)]
            StorageBackend::Remote(connection) => Ok(StorageTransaction {
                backend: StorageTransactionBackend::Remote(
                    connection.transaction_with_behavior(behavior)?,
                ),
            }),
        }
    }

    /// Begins a transaction without checking SQLite's nested-transaction state first.
    pub(crate) fn unchecked_transaction(&mut self) -> Result<StorageTransaction<'_>> {
        match &mut self.backend {
            StorageBackend::Local(connection) => Ok(StorageTransaction {
                backend: StorageTransactionBackend::Local(connection.unchecked_transaction()?),
            }),
            #[cfg(test)]
            StorageBackend::Standalone(connection) => Ok(StorageTransaction {
                backend: StorageTransactionBackend::Local(connection.unchecked_transaction()?),
            }),
            #[cfg(windows)]
            StorageBackend::Remote(connection) => Ok(StorageTransaction {
                backend: StorageTransactionBackend::Remote(connection.transaction()?),
            }),
        }
    }

    #[cfg(windows)]
    pub(crate) fn is_remote(&self) -> bool {
        matches!(&self.backend, StorageBackend::Remote(_))
    }

    #[cfg(windows)]
    pub(crate) fn vault_read_map(&self) -> Result<crate::secrets::SecretMap> {
        match &self.backend {
            StorageBackend::Remote(connection) => connection.vault_read_map(),
            _ => Err(Error::InvalidQuery),
        }
    }

    #[cfg(windows)]
    pub(crate) fn vault_write_map(&self, secrets: crate::secrets::SecretMap) -> Result<()> {
        match &self.backend {
            StorageBackend::Remote(connection) => connection.vault_write_map(secrets),
            _ => Err(Error::InvalidQuery),
        }
    }

    #[cfg(windows)]
    pub(crate) fn vault_get(&self, key: &str) -> Result<Option<zeroize::Zeroizing<String>>> {
        match &self.backend {
            StorageBackend::Remote(connection) => connection.vault_get(key),
            _ => Err(Error::InvalidQuery),
        }
    }

    #[cfg(windows)]
    pub(crate) fn vault_set(&self, key: &str, value: &str) -> Result<()> {
        match &self.backend {
            StorageBackend::Remote(connection) => connection.vault_set(key, value),
            _ => Err(Error::InvalidQuery),
        }
    }

    #[cfg(windows)]
    pub(crate) fn vault_delete(&self, key: &str) -> Result<()> {
        match &self.backend {
            StorageBackend::Remote(connection) => connection.vault_delete(key),
            _ => Err(Error::InvalidQuery),
        }
    }

    #[cfg(windows)]
    pub(crate) fn vault_has(&self, key: &str) -> Result<bool> {
        match &self.backend {
            StorageBackend::Remote(connection) => connection.vault_has(key),
            _ => Err(Error::InvalidQuery),
        }
    }

    /// Registers the credential-redaction function for a local connection.
    ///
    /// The Windows broker registers this function while opening its service-owned SQLite
    /// connection, so the client side intentionally treats registration as already satisfied.
    pub(crate) fn register_tool_index_redaction(&self) -> Result<()> {
        match &self.backend {
            StorageBackend::Local(connection) => {
                super::tool_index_redaction::register_sqlite_function(connection)
            }
            #[cfg(test)]
            StorageBackend::Standalone(connection) => {
                super::tool_index_redaction::register_sqlite_function(connection)
            }
            #[cfg(windows)]
            StorageBackend::Remote(_) => Ok(()),
        }
    }

    /// Checks database integrity through the active storage backend.
    pub(crate) fn verify_integrity(&self) -> Result<()> {
        match &self.backend {
            StorageBackend::Local(connection) => {
                super::database_guard::verify_integrity(connection)
            }
            #[cfg(test)]
            StorageBackend::Standalone(connection) => {
                super::database_guard::verify_integrity(connection)
            }
            #[cfg(windows)]
            StorageBackend::Remote(connection) => {
                let cipher_statement = connection.prepare("PRAGMA cipher_integrity_check")?;
                let mut cipher_rows = cipher_statement.query([])?;
                if cipher_rows.next()?.is_some() {
                    return Err(Error::InvalidQuery);
                }
                drop(cipher_rows);
                drop(cipher_statement);

                let result = connection
                    .query_row("PRAGMA quick_check(1)", [], |row| row.get::<_, String>(0))?;
                if result.eq_ignore_ascii_case("ok") {
                    Ok(())
                } else {
                    Err(Error::InvalidQuery)
                }
            }
        }
    }

    /// Writes a verified database snapshot when the active backend supports local backup.
    pub(crate) fn create_verified_snapshot(&self, destination: &Path) -> Result<()> {
        match &self.backend {
            StorageBackend::Local(connection) => {
                let database_key = self.database_key.ok_or(Error::InvalidQuery)?;
                super::database_guard::create_verified_snapshot(
                    connection,
                    destination,
                    database_key,
                )
            }
            #[cfg(test)]
            StorageBackend::Standalone(_) => Err(Error::InvalidQuery),
            #[cfg(windows)]
            StorageBackend::Remote(connection) => connection.create_verified_snapshot(destination),
        }
    }
}

/// A prepared statement whose result data is copied into [`StorageRow`] values.
pub(crate) struct StorageStatement<'a> {
    backend: StorageStatementBackend<'a>,
}

enum StorageStatementBackend<'a> {
    Local(Statement<'a>),
    LocalCached(CachedStatement<'a>),
    #[cfg(windows)]
    Remote(BrokerStatement),
}

impl StorageStatement<'_> {
    /// Executes the statement and returns its changed-row count.
    pub(crate) fn execute<P: Params>(&mut self, params: P) -> Result<usize> {
        match &mut self.backend {
            StorageStatementBackend::Local(statement) => statement.execute(params),
            StorageStatementBackend::LocalCached(statement) => statement.execute(params),
            #[cfg(windows)]
            StorageStatementBackend::Remote(statement) => statement.execute(params),
        }
    }

    /// Executes the statement and materializes its rows.
    pub(crate) fn query<P: Params>(&mut self, params: P) -> Result<StorageRows> {
        match &mut self.backend {
            StorageStatementBackend::Local(statement) => collect_local_rows(statement, params),
            StorageStatementBackend::LocalCached(statement) => {
                collect_local_cached_rows(statement, params)
            }
            #[cfg(windows)]
            StorageStatementBackend::Remote(statement) => {
                let rows = statement.query(params)?;
                collect_remote_rows(rows)
            }
        }
    }

    /// Executes the statement and maps its owned rows.
    pub(crate) fn query_map<T, P, F>(&mut self, params: P, f: F) -> Result<StorageMappedRows<T, F>>
    where
        P: Params,
        F: FnMut(&StorageRow) -> Result<T>,
    {
        Ok(StorageMappedRows {
            rows: self.query(params)?,
            map: f,
            marker: PhantomData,
        })
    }

    /// Executes the statement and maps its first row.
    pub(crate) fn query_row<T, P, F>(&mut self, params: P, f: F) -> Result<T>
    where
        P: Params,
        F: FnOnce(&StorageRow) -> Result<T>,
    {
        let mut rows = self.query(params)?;
        let row = rows.next()?.ok_or(Error::QueryReturnedNoRows)?;
        f(row)
    }

    /// Checks whether the statement returns at least one row.
    pub(crate) fn exists<P: Params>(&mut self, params: P) -> Result<bool> {
        let mut rows = self.query(params)?;
        Ok(rows.next()?.is_some())
    }
}

/// A transaction tied to a [`StorageConnection`] lease.
pub(crate) struct StorageTransaction<'a> {
    backend: StorageTransactionBackend<'a>,
}

enum StorageTransactionBackend<'a> {
    Local(Transaction<'a>),
    #[cfg(windows)]
    Remote(BrokerTransaction),
}

impl StorageTransaction<'_> {
    /// Executes one statement inside the transaction.
    pub(crate) fn execute<P: Params>(&self, sql: &str, params: P) -> Result<usize> {
        match &self.backend {
            StorageTransactionBackend::Local(transaction) => transaction.execute(sql, params),
            #[cfg(windows)]
            StorageTransactionBackend::Remote(transaction) => transaction.execute(sql, params),
        }
    }

    /// Executes a batch inside the transaction.
    pub(crate) fn execute_batch(&self, sql: &str) -> Result<()> {
        match &self.backend {
            StorageTransactionBackend::Local(transaction) => transaction.execute_batch(sql),
            #[cfg(windows)]
            StorageTransactionBackend::Remote(transaction) => transaction.execute_batch(sql),
        }
    }

    /// Runs a query inside the transaction and materializes its rows.
    pub(crate) fn query<P: Params>(&self, sql: &str, params: P) -> Result<StorageRows> {
        let mut statement = self.prepare(sql)?;
        statement.query(params)
    }

    /// Runs a query expected to return at least one row.
    pub(crate) fn query_row<T, P, F>(&self, sql: &str, params: P, f: F) -> Result<T>
    where
        P: Params,
        F: FnOnce(&StorageRow) -> Result<T>,
    {
        let mut statement = self.prepare(sql)?;
        statement.query_row(params, f)
    }

    /// Prepares a statement for use inside this transaction.
    pub(crate) fn prepare(&self, sql: &str) -> Result<StorageStatement<'_>> {
        match &self.backend {
            StorageTransactionBackend::Local(transaction) => Ok(StorageStatement {
                backend: StorageStatementBackend::Local(transaction.prepare(sql)?),
            }),
            #[cfg(windows)]
            StorageTransactionBackend::Remote(transaction) => Ok(StorageStatement {
                backend: StorageStatementBackend::Remote(transaction.prepare(sql)?),
            }),
        }
    }

    /// Prepares a cached statement for use inside this transaction.
    pub(crate) fn prepare_cached(&self, sql: &str) -> Result<StorageStatement<'_>> {
        match &self.backend {
            StorageTransactionBackend::Local(transaction) => Ok(StorageStatement {
                backend: StorageStatementBackend::LocalCached(transaction.prepare_cached(sql)?),
            }),
            #[cfg(windows)]
            StorageTransactionBackend::Remote(transaction) => Ok(StorageStatement {
                backend: StorageStatementBackend::Remote(transaction.prepare(sql)?),
            }),
        }
    }

    /// Commits the transaction.
    pub(crate) fn commit(self) -> Result<()> {
        match self.backend {
            StorageTransactionBackend::Local(transaction) => transaction.commit(),
            #[cfg(windows)]
            StorageTransactionBackend::Remote(transaction) => transaction.commit(),
        }
    }

    /// Rolls back the transaction.
    pub(crate) fn rollback(self) -> Result<()> {
        match self.backend {
            StorageTransactionBackend::Local(transaction) => transaction.rollback(),
            #[cfg(windows)]
            StorageTransactionBackend::Remote(transaction) => transaction.rollback(),
        }
    }

    /// Returns the rowid reported by the last successful insert.
    pub(crate) fn last_insert_rowid(&self) -> Result<i64> {
        match &self.backend {
            StorageTransactionBackend::Local(transaction) => Ok(transaction.last_insert_rowid()),
            #[cfg(windows)]
            StorageTransactionBackend::Remote(transaction) => transaction.last_insert_rowid(),
        }
    }
}

/// An owned result row with SQLite-compatible typed extraction.
pub(crate) struct StorageRow {
    columns: Arc<[String]>,
    values: Vec<Value>,
}

impl StorageRow {
    fn new(columns: Arc<[String]>, values: Vec<Value>) -> Self {
        Self { columns, values }
    }

    /// Reads one column using a numeric index or case-insensitive column name.
    pub(crate) fn get<I, T>(&self, index: I) -> Result<T>
    where
        I: StorageRowIndex,
        T: FromSql,
    {
        let (index, value) = self.value(index)?;
        let data_type = value.data_type();
        T::column_result(ValueRef::from(value)).map_err(|error| match error {
            FromSqlError::InvalidType => {
                Error::InvalidColumnType(index, self.columns[index].clone(), data_type)
            }
            FromSqlError::OutOfRange(value) => Error::IntegralValueOutOfRange(index, value),
            FromSqlError::Utf8Error(error) => Error::Utf8Error(index, error),
            other => Error::FromSqlConversionFailure(index, data_type, Box::new(other)),
        })
    }

    /// Borrows one non-null text column without cloning its owned value.
    pub(crate) fn get_str<I>(&self, index: I) -> Result<&str>
    where
        I: StorageRowIndex,
    {
        let (index, value) = self.value(index)?;
        match value {
            Value::Text(value) => Ok(value),
            value => Err(Error::InvalidColumnType(
                index,
                self.columns[index].clone(),
                value.data_type(),
            )),
        }
    }

    /// Borrows one nullable text column without cloning its owned value.
    pub(crate) fn get_optional_str<I>(&self, index: I) -> Result<Option<&str>>
    where
        I: StorageRowIndex,
    {
        let (index, value) = self.value(index)?;
        match value {
            Value::Null => Ok(None),
            Value::Text(value) => Ok(Some(value)),
            value => Err(Error::InvalidColumnType(
                index,
                self.columns[index].clone(),
                value.data_type(),
            )),
        }
    }

    fn value<I>(&self, index: I) -> Result<(usize, &Value)>
    where
        I: StorageRowIndex,
    {
        let index = index.resolve(&self.columns)?;
        let value = self
            .values
            .get(index)
            .ok_or(Error::InvalidColumnIndex(index))?;
        Ok((index, value))
    }
}

/// Resolves the row index forms used by the storage callers.
pub(crate) trait StorageRowIndex {
    fn resolve(&self, columns: &[String]) -> Result<usize>;
}

impl StorageRowIndex for usize {
    fn resolve(&self, columns: &[String]) -> Result<usize> {
        if *self < columns.len() {
            Ok(*self)
        } else {
            Err(Error::InvalidColumnIndex(*self))
        }
    }
}

impl StorageRowIndex for &str {
    fn resolve(&self, columns: &[String]) -> Result<usize> {
        columns
            .iter()
            .position(|column| column.eq_ignore_ascii_case(self))
            .ok_or_else(|| Error::InvalidColumnName((*self).to_owned()))
    }
}

/// An owned result row stream.  The current row remains borrowed until the next call to
/// [`StorageRows::next`], matching the existing broker facade and rusqlite cursor usage.
pub(crate) struct StorageRows {
    rows: std::vec::IntoIter<StorageRow>,
    current: Option<StorageRow>,
}

impl StorageRows {
    fn new(rows: Vec<StorageRow>) -> Self {
        Self {
            rows: rows.into_iter(),
            current: None,
        }
    }

    /// Returns the next owned row, borrowing it until the next call.
    pub(crate) fn next(&mut self) -> Result<Option<&StorageRow>> {
        self.current = self.rows.next();
        Ok(self.current.as_ref())
    }
}

/// An iterator over rows mapped by a caller-provided fallible function.
pub(crate) struct StorageMappedRows<T, F> {
    rows: StorageRows,
    map: F,
    marker: PhantomData<fn() -> T>,
}

impl<T, F> Iterator for StorageMappedRows<T, F>
where
    F: FnMut(&StorageRow) -> Result<T>,
{
    type Item = Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.rows.next() {
            Ok(Some(row)) => Some((self.map)(row)),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        }
    }
}

fn collect_local_rows<P: Params>(statement: &mut Statement<'_>, params: P) -> Result<StorageRows> {
    let columns = statement
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    ensure_result_shape(columns.len(), 0)?;
    let columns: Arc<[String]> = Arc::from(columns.into_boxed_slice());
    let mut cursor = statement.query(params)?;
    let mut rows = Vec::new();
    while let Some(row) = cursor.next()? {
        if rows.len() >= MAX_STORAGE_ROWS {
            return Err(result_too_large(
                "The storage query returned too many rows.",
            ));
        }
        let values = (0..columns.len())
            .map(|index| row.get::<_, Value>(index))
            .collect::<Result<Vec<_>>>()?;
        rows.push(StorageRow::new(Arc::clone(&columns), values));
    }
    Ok(StorageRows::new(rows))
}

fn collect_local_cached_rows<P: Params>(
    statement: &mut CachedStatement<'_>,
    params: P,
) -> Result<StorageRows> {
    let columns = statement
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    ensure_result_shape(columns.len(), 0)?;
    let columns: Arc<[String]> = Arc::from(columns.into_boxed_slice());
    let mut cursor = statement.query(params)?;
    let mut rows = Vec::new();
    while let Some(row) = cursor.next()? {
        if rows.len() >= MAX_STORAGE_ROWS {
            return Err(result_too_large(
                "The storage query returned too many rows.",
            ));
        }
        let values = (0..columns.len())
            .map(|index| row.get::<_, Value>(index))
            .collect::<Result<Vec<_>>>()?;
        rows.push(StorageRow::new(Arc::clone(&columns), values));
    }
    Ok(StorageRows::new(rows))
}

#[cfg(windows)]
fn collect_remote_rows(mut remote_rows: BrokerRows) -> Result<StorageRows> {
    let mut rows = Vec::new();
    let mut columns: Option<Arc<[String]>> = None;
    while let Some(remote_row) = remote_rows.next()? {
        if rows.len() >= MAX_STORAGE_ROWS {
            return Err(result_too_large(
                "The storage query returned too many rows.",
            ));
        }
        let column_count = remote_row.column_count();
        ensure_result_shape(column_count, rows.len())?;
        let row_columns = if let Some(columns) = &columns {
            Arc::clone(columns)
        } else {
            let names = (0..column_count)
                .map(|index| remote_row.column_name(index).map(str::to_owned))
                .collect::<Result<Vec<_>>>()?;
            let names: Arc<[String]> = Arc::from(names.into_boxed_slice());
            columns = Some(Arc::clone(&names));
            names
        };
        let values = (0..column_count)
            .map(|index| remote_row.get::<_, Value>(index))
            .collect::<Result<Vec<_>>>()?;
        rows.push(StorageRow::new(row_columns, values));
    }
    Ok(StorageRows::new(rows))
}

fn ensure_result_shape(column_count: usize, _row_count: usize) -> Result<()> {
    if column_count > MAX_STORAGE_COLUMNS {
        return Err(result_too_large(
            "The storage query returned too many columns.",
        ));
    }
    Ok(())
}

fn result_too_large(message: &'static str) -> Error {
    Error::SqliteFailure(
        ffi::Error::new(ffi::SQLITE_TOOBIG),
        Some(message.to_owned()),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rusqlite::{Error, types::Value};

    use super::{StorageRow, StorageRows};

    fn row(text: &str) -> StorageRow {
        StorageRow::new(
            Arc::from(vec!["value".to_owned(), "optional".to_owned()].into_boxed_slice()),
            vec![Value::Text(text.to_owned()), Value::Null],
        )
    }

    #[test]
    fn borrowed_text_accessors_return_text_and_null() {
        let row = row("value");

        assert_eq!(row.get_str("value").unwrap(), "value");
        assert_eq!(row.get_optional_str("optional").unwrap(), None);
    }

    #[test]
    fn borrowed_text_accessor_preserves_typed_column_errors() {
        let row = row("value");

        assert!(matches!(
            row.get_str("optional"),
            Err(Error::InvalidColumnType(1, name, rusqlite::types::Type::Null))
                if name == "optional"
        ));
    }

    #[test]
    fn storage_rows_advance_without_changing_the_borrowed_row_contract() {
        let mut rows = StorageRows::new(vec![row("first"), row("second")]);

        assert_eq!(rows.next().unwrap().unwrap().get_str(0).unwrap(), "first");
        assert_eq!(rows.next().unwrap().unwrap().get_str(0).unwrap(), "second");
        assert!(rows.next().unwrap().is_none());
    }
}
