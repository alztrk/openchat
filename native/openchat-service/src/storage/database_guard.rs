use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(test)]
use rusqlite::ErrorCode;
use rusqlite::{
    Connection, Error,
    backup::{Backup, StepResult},
};
use uuid::Uuid;

const BACKUP_STEP_PAGES: i32 = 128;
const BACKUP_RETRY_DELAY: Duration = Duration::from_millis(50);
const BACKUP_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn verify_integrity(connection: &Connection) -> rusqlite::Result<()> {
    let result: String = connection.query_row("PRAGMA quick_check(1)", [], |row| row.get(0))?;
    if result == "ok" {
        return Ok(());
    }

    Err(corrupt_database_error())
}

#[cfg(test)]
pub(super) fn is_corrupt_database_error(error: &Error) -> bool {
    matches!(
        error,
        Error::SqliteFailure(failure, _)
            if matches!(failure.code, ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase)
    )
}

pub(super) fn backend_schema_version(connection: &Connection) -> rusqlite::Result<i64> {
    let exists = connection.query_row(
        "SELECT EXISTS (
            SELECT 1 FROM sqlite_schema
            WHERE type = 'table' AND name = 'openchat_backend_migrations'
        )",
        [],
        |row| row.get::<_, bool>(0),
    )?;
    if !exists {
        return Ok(0);
    }

    connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM openchat_backend_migrations",
        [],
        |row| row.get(0),
    )
}

pub(super) fn chat_schema_version(connection: &Connection) -> rusqlite::Result<i64> {
    connection.query_row("PRAGMA user_version", [], |row| row.get(0))
}

pub(super) fn create_verified_backup(
    source: &Connection,
    database_path: &Path,
) -> rusqlite::Result<PathBuf> {
    verify_integrity(source)?;

    let database_directory = database_path
        .parent()
        .ok_or_else(|| Error::InvalidPath(database_path.to_path_buf()))?;
    let backup_directory = database_directory.join("backups");
    fs::create_dir_all(&backup_directory).map_err(io_error)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Error::ToSqlConversionFailure(Box::new(error)))?
        .as_millis();
    let name = format!(
        "openchat.sqlite3.pre-migration-{timestamp}-{}.backup",
        Uuid::new_v4()
    );
    let final_path = backup_directory.join(name);
    let temporary_path = final_path.with_extension("backup.partial");

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)
        .map_err(io_error)?;

    let backup_result = build_backup(source, &temporary_path);
    if let Err(error) = backup_result {
        match fs::remove_file(&temporary_path) {
            Ok(()) => return Err(error),
            Err(cleanup_error) if cleanup_error.kind() == io::ErrorKind::NotFound => {
                return Err(error);
            }
            Err(cleanup_error) => return Err(io_error(cleanup_error)),
        }
    }

    if let Err(error) = fs::rename(&temporary_path, &final_path) {
        match fs::remove_file(&temporary_path) {
            Ok(()) => return Err(io_error(error)),
            Err(cleanup_error) if cleanup_error.kind() == io::ErrorKind::NotFound => {
                return Err(io_error(error));
            }
            Err(cleanup_error) => return Err(io_error(cleanup_error)),
        }
    }
    Ok(final_path)
}

fn build_backup(source: &Connection, temporary_path: &Path) -> rusqlite::Result<()> {
    let mut destination = Connection::open(temporary_path)?;
    {
        let backup = Backup::new(source, &mut destination)?;
        let started_at = Instant::now();
        loop {
            match backup.step(BACKUP_STEP_PAGES)? {
                StepResult::Done => break,
                StepResult::More => {}
                StepResult::Busy | StepResult::Locked => {
                    if started_at.elapsed() >= BACKUP_TIMEOUT {
                        return Err(sqlite_error(
                            rusqlite::ffi::SQLITE_BUSY,
                            "Database backup timed out while waiting for a lock.",
                        ));
                    }
                    thread::sleep(BACKUP_RETRY_DELAY);
                }
                _ => return Err(Error::InvalidQuery),
            }
        }
    }

    verify_integrity(&destination)?;
    destination.close().map_err(|(_, error)| error)
}

fn corrupt_database_error() -> Error {
    sqlite_error(
        rusqlite::ffi::SQLITE_CORRUPT,
        "Database integrity check failed.",
    )
}

fn sqlite_error(code: i32, message: &'static str) -> Error {
    Error::SqliteFailure(rusqlite::ffi::Error::new(code), Some(message.to_owned()))
}

fn io_error(error: io::Error) -> Error {
    Error::ToSqlConversionFailure(Box::new(error))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::{create_verified_backup, is_corrupt_database_error, verify_integrity};
    use rusqlite::Connection;
    use uuid::Uuid;

    #[test]
    fn bundled_sqlite_includes_the_wal_reset_fix() {
        assert!(
            rusqlite::version_number() >= 3_051_003,
            "bundled SQLite {} predates the WAL-reset fix",
            rusqlite::version()
        );
    }

    #[test]
    fn verified_backup_contains_committed_wal_data() {
        let directory = TestDirectory::new();
        let database_path = directory.0.join("openchat.sqlite3");
        let connection = Connection::open(&database_path).expect("open source database");
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 CREATE TABLE backup_probe (value TEXT NOT NULL);
                 INSERT INTO backup_probe VALUES ('preserved');",
            )
            .expect("create committed WAL content");

        let backup_path = create_verified_backup(&connection, &database_path)
            .expect("create and validate backup");
        let backup = Connection::open(&backup_path).expect("open backup database");
        let value = backup
            .query_row("SELECT value FROM backup_probe", [], |row| {
                row.get::<_, String>(0)
            })
            .expect("read backed-up WAL content");

        assert_eq!(value, "preserved");
        verify_integrity(&backup).expect("backup passes integrity check");
        assert!(backup_path.starts_with(directory.0.join("backups")));
    }

    #[test]
    fn integrity_check_rejects_non_database_without_changing_source() {
        let directory = TestDirectory::new();
        let database_path = directory.0.join("openchat.sqlite3");
        let original = b"not an sqlite database";
        fs::write(&database_path, original).expect("write corrupt fixture");
        let connection = Connection::open(&database_path).expect("open corrupt fixture");

        let error = verify_integrity(&connection).expect_err("reject corrupt database");

        assert!(is_corrupt_database_error(&error));
        assert_eq!(fs::read(database_path).expect("read fixture"), original);
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("openchat-database-guard-{}", Uuid::new_v4()));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test directory");
        }
    }
}
