//! Handles SQLCipher keys without exposing them through storage diagnostics.

use std::{
    fmt::Write as _,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use ring::rand::{SecureRandom, SystemRandom};
use rusqlite::{Connection, OpenFlags, params};
use zeroize::Zeroizing;

const DATABASE_KEY_BYTES: usize = 32;
const MIGRATION_JOURNAL_CONTENT: &[u8] = b"OpenChat SQLCipher migration v1\n";
#[cfg(feature = "desktop-profile-test-support")]
const MAXIMUM_DESKTOP_TEST_PROFILE_ENTRIES: usize = 65_536;

pub(crate) struct DatabaseKey(Zeroizing<[u8; DATABASE_KEY_BYTES]>);

impl DatabaseKey {
    pub(crate) fn generate() -> rusqlite::Result<Self> {
        let mut bytes = Zeroizing::new([0; DATABASE_KEY_BYTES]);
        SystemRandom::new()
            .fill(&mut *bytes)
            .map_err(|_| encryption_error())?;
        Ok(Self(bytes))
    }

    pub(crate) fn from_bytes(bytes: &[u8]) -> rusqlite::Result<Self> {
        let key: [u8; DATABASE_KEY_BYTES] = bytes.try_into().map_err(|_| encryption_error())?;
        Ok(Self(Zeroizing::new(key)))
    }

    pub(crate) fn as_bytes(&self) -> &[u8; DATABASE_KEY_BYTES] {
        &self.0
    }

    pub(crate) fn open(&self, path: &Path, flags: OpenFlags) -> rusqlite::Result<Connection> {
        let connection = Connection::open_with_flags(path, flags)?;
        self.apply(&connection)?;
        Ok(connection)
    }

    pub(crate) fn open_or_migrate(&self, path: &Path) -> rusqlite::Result<(Connection, bool)> {
        self.recover_plaintext_migration(path)?;
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(rusqlite::Error::InvalidPath(path.to_path_buf()));
            }
            #[cfg(feature = "desktop-profile-test-support")]
            Ok(metadata) if is_hard_link(path, &metadata) => {
                return Err(rusqlite::Error::InvalidPath(path.to_path_buf()));
            }
            Ok(metadata) if metadata.len() >= SQLITE_HEADER.len() as u64 => {
                let mut file = File::open(path).map_err(filesystem_error)?;
                let mut header = [0; SQLITE_HEADER.len()];
                file.read_exact(&mut header).map_err(filesystem_error)?;
                if &header == SQLITE_HEADER {
                    self.migrate_plaintext_database(path)?;
                    return self
                        .open(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
                        .map(|connection| (connection, true));
                }
                return self
                    .open(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
                    .map(|connection| (connection, false));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(filesystem_error(error)),
        }

        self.open(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .map(|connection| (connection, false))
    }

    pub(crate) fn has_recoverable_plaintext_migration(
        database_path: &Path,
    ) -> rusqlite::Result<bool> {
        let paths = migration_paths(database_path);
        let journal_metadata = match fs::symlink_metadata(&paths.journal) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(filesystem_error(error)),
        };
        if !journal_metadata.is_file()
            || journal_metadata.file_type().is_symlink()
            || journal_metadata.len() != MIGRATION_JOURNAL_CONTENT.len() as u64
        {
            return Err(rusqlite::Error::InvalidPath(paths.journal));
        }
        if fs::read(&paths.journal).map_err(filesystem_error)? != MIGRATION_JOURNAL_CONTENT {
            return Err(encryption_error());
        }
        if !is_regular_file(&paths.plaintext_source)? {
            return Ok(false);
        }

        let mut source = File::open(&paths.plaintext_source).map_err(filesystem_error)?;
        let mut header = [0; SQLITE_HEADER.len()];
        match source.read_exact(&mut header) {
            Ok(()) => Ok(&header == SQLITE_HEADER),
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
            Err(error) => Err(filesystem_error(error)),
        }
    }

    pub(super) fn apply(&self, connection: &Connection) -> rusqlite::Result<()> {
        let mut statement = Zeroizing::new(String::with_capacity(96));
        statement.push_str("PRAGMA key = \"x'");
        for byte in self.0.iter() {
            write!(statement, "{byte:02x}").map_err(|_| encryption_error())?;
        }
        statement.push_str("'\";");
        connection
            .execute_batch(&statement)
            .map_err(|_| encryption_error())?;

        let cipher_version = connection
            .query_row("PRAGMA cipher_version", [], |row| row.get::<_, String>(0))
            .map_err(|_| encryption_error())?;
        if cipher_version.trim().is_empty() {
            return Err(encryption_error());
        }

        connection
            .query_row("SELECT count(*) FROM sqlite_master", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|_| encryption_error())?;
        Ok(())
    }

    fn migrate_plaintext_database(&self, path: &Path) -> rusqlite::Result<()> {
        let paths = migration_paths(path);
        write_migration_journal(&paths.journal)?;
        if let Err(error) = self.export_plaintext_database(path, &paths.encrypted_partial) {
            remove_if_present(&paths.journal)?;
            return Err(error);
        }

        if let Err(error) = fs::rename(path, &paths.plaintext_source) {
            remove_if_present(&paths.encrypted_partial)?;
            remove_if_present(&paths.journal)?;
            return Err(filesystem_error(error));
        }
        for (source, destination) in sidecar_paths(path)
            .iter()
            .zip(sidecar_paths(&paths.plaintext_source).iter())
        {
            if let Err(error) = rename_if_present(source, destination) {
                self.recover_plaintext_migration(path)?;
                return Err(error);
            }
        }
        if let Err(error) = fs::rename(&paths.encrypted_partial, path) {
            self.recover_plaintext_migration(path)?;
            return Err(filesystem_error(error));
        }

        let validation = self
            .open(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .and_then(|connection| super::database_guard::verify_integrity(&connection));
        if let Err(error) = validation {
            self.recover_plaintext_migration(path)?;
            return Err(error);
        }
        self.cleanup_completed_plaintext_migration(&paths)
    }

    fn recover_plaintext_migration(&self, path: &Path) -> rusqlite::Result<()> {
        let paths = migration_paths(path);
        let journal_metadata = match fs::symlink_metadata(&paths.journal) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(filesystem_error(error)),
        };
        if !journal_metadata.is_file() || journal_metadata.file_type().is_symlink() {
            return Err(rusqlite::Error::InvalidPath(paths.journal));
        }
        let journal = fs::read(&paths.journal).map_err(filesystem_error)?;
        if journal != MIGRATION_JOURNAL_CONTENT {
            return Err(encryption_error());
        }

        let source_exists = is_regular_file(&paths.plaintext_source)?;
        if source_exists {
            if self.encrypted_database_is_valid(path) {
                return self.cleanup_completed_plaintext_migration(&paths);
            }

            if is_regular_file(path)? {
                let failed_path = append_suffix(path, ".sqlcipher-failed");
                if is_regular_file(&failed_path)? {
                    return Err(rusqlite::Error::InvalidPath(failed_path));
                }
                fs::rename(path, &failed_path).map_err(filesystem_error)?;
            }
            fs::rename(&paths.plaintext_source, path).map_err(filesystem_error)?;
            for (source, destination) in sidecar_paths(&paths.plaintext_source)
                .iter()
                .zip(sidecar_paths(path).iter())
            {
                rename_if_present(source, destination)?;
            }
            remove_if_present(&paths.encrypted_partial)?;
            remove_if_present(&paths.journal)
        } else if is_regular_file(path)? {
            if self.is_plaintext_database(path)? {
                remove_if_present(&paths.encrypted_partial)?;
                remove_if_present(&paths.journal)
            } else if self.encrypted_database_is_valid(path) {
                self.cleanup_completed_plaintext_migration(&paths)
            } else {
                Err(encryption_error())
            }
        } else {
            Err(encryption_error())
        }
    }

    fn encrypted_database_is_valid(&self, path: &Path) -> bool {
        self.open(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .and_then(|connection| super::database_guard::verify_integrity(&connection))
            .is_ok()
    }

    fn is_plaintext_database(&self, path: &Path) -> rusqlite::Result<bool> {
        let mut file = File::open(path).map_err(filesystem_error)?;
        let mut header = [0; SQLITE_HEADER.len()];
        file.read_exact(&mut header).map_err(filesystem_error)?;
        Ok(&header == SQLITE_HEADER)
    }

    fn cleanup_completed_plaintext_migration(
        &self,
        paths: &MigrationPaths,
    ) -> rusqlite::Result<()> {
        remove_if_present(&paths.plaintext_source)?;
        let source_sidecars = sidecar_paths(&paths.plaintext_source);
        let partial_sidecars = sidecar_paths(&paths.encrypted_partial);
        for sidecar in source_sidecars.iter().chain(partial_sidecars.iter()) {
            remove_if_present(sidecar)?;
        }
        remove_if_present(&paths.encrypted_partial)?;
        remove_if_present(&paths.journal)
    }

    pub(crate) fn export_plaintext_database(
        &self,
        source_path: &Path,
        encrypted_path: &Path,
    ) -> rusqlite::Result<()> {
        let mut output = CreatedFile::create(encrypted_path)?;
        let result = (|| {
            let source =
                Connection::open_with_flags(source_path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
            super::disable_trusted_schema(&source)?;
            super::database_guard::verify_plaintext_integrity(&source)?;
            self.export_attached_database(&source, encrypted_path)?;
            drop(source);

            let encrypted = self.open(encrypted_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            super::database_guard::verify_integrity(&encrypted)?;
            drop(encrypted);
            let mut file = File::open(encrypted_path).map_err(filesystem_error)?;
            let mut header = [0; SQLITE_HEADER.len()];
            file.read_exact(&mut header).map_err(filesystem_error)?;
            if &header == SQLITE_HEADER {
                return Err(encryption_error());
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                output.disarm();
                Ok(())
            }
            Err(error) => {
                output.remove()?;
                Err(error)
            }
        }
    }

    pub(crate) fn export_encrypted_database(
        &self,
        source_path: &Path,
        destination_path: &Path,
        destination_key: &DatabaseKey,
    ) -> rusqlite::Result<()> {
        let mut output = CreatedFile::create(destination_path)?;
        let result = (|| {
            let source = self.open(source_path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
            super::disable_trusted_schema(&source)?;
            super::database_guard::verify_integrity(&source)?;
            destination_key.export_attached_database(&source, destination_path)?;
            drop(source);

            let destination =
                destination_key.open(destination_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            super::database_guard::verify_integrity(&destination)
        })();
        match result {
            Ok(()) => {
                output.disarm();
                Ok(())
            }
            Err(error) => {
                output.remove()?;
                Err(error)
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn export_test_plaintext_database(
        &self,
        source: &Connection,
        plaintext_path: &Path,
    ) -> rusqlite::Result<()> {
        let mut output = CreatedFile::create(plaintext_path)?;
        let result = (|| {
            super::database_guard::verify_integrity(source)?;
            let destination = plaintext_path.to_string_lossy();
            source.execute(
                "ATTACH DATABASE ?1 AS plaintext KEY ''",
                [destination.as_ref()],
            )?;
            source.execute_batch("SELECT sqlcipher_export('plaintext');")?;
            for pragma in ["user_version", "application_id"] {
                let value: i64 =
                    source.query_row(&format!("PRAGMA {pragma}"), [], |row| row.get(0))?;
                source.pragma_update(Some("plaintext"), pragma, value)?;
            }
            source.execute_batch("DETACH DATABASE plaintext;")?;
            let database =
                Connection::open_with_flags(plaintext_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            super::database_guard::verify_plaintext_integrity(&database)
        })();
        match result {
            Ok(()) => {
                output.disarm();
                Ok(())
            }
            Err(error) => {
                output.remove()?;
                Err(error)
            }
        }
    }

    fn export_attached_database(
        &self,
        source: &Connection,
        destination_path: &Path,
    ) -> rusqlite::Result<()> {
        let key_value = self.as_pragma_value();
        let destination = destination_path.to_string_lossy();
        source.execute(
            "ATTACH DATABASE ?1 AS encrypted KEY ?2",
            params![destination.as_ref(), key_value.as_str()],
        )?;
        source.execute_batch("SELECT sqlcipher_export('encrypted');")?;
        for pragma in ["user_version", "application_id"] {
            let value: i64 = source.query_row(&format!("PRAGMA {pragma}"), [], |row| row.get(0))?;
            source.pragma_update(Some("encrypted"), pragma, value)?;
        }
        source.execute_batch("DETACH DATABASE encrypted;")
    }

    fn as_pragma_value(&self) -> Zeroizing<String> {
        let mut value = Zeroizing::new(String::with_capacity(67));
        value.push_str("x'");
        for byte in self.0.iter() {
            let _ = write!(value, "{byte:02x}");
        }
        value.push('\'');
        value
    }

    #[cfg(test)]
    pub(super) fn isolated_test_key() -> Self {
        Self(Zeroizing::new([0xA5; DATABASE_KEY_BYTES]))
    }
}

#[cfg(feature = "desktop-profile-test-support")]
pub(super) fn read_or_create_desktop_test_key(
    storage_root: &Path,
) -> rusqlite::Result<DatabaseKey> {
    let profile_root = validate_desktop_test_profile_root(storage_root)?;
    let key_path = profile_root.join("database-test-key.bin");
    if let Some(key) = read_desktop_test_key(&key_path)? {
        return Ok(key);
    }

    let key = DatabaseKey::generate()?;
    let partial_path = profile_root.join(format!(
        ".database-test-key-{}.tmp",
        uuid::Uuid::new_v4().simple()
    ));
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial_path)
            .map_err(filesystem_error)?;
        file.write_all(key.as_bytes()).map_err(filesystem_error)?;
        file.sync_all().map_err(filesystem_error)
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&partial_path);
        return Err(error);
    }

    let publish_result = fs::hard_link(&partial_path, &key_path);
    let cleanup_result = fs::remove_file(&partial_path).map_err(filesystem_error);
    if let Err(error) = cleanup_result {
        return Err(error);
    }
    match publish_result {
        Ok(()) => Ok(key),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            read_desktop_test_key(&key_path)?.ok_or_else(encryption_error)
        }
        Err(error) => Err(filesystem_error(error)),
    }
}

#[cfg(feature = "desktop-profile-test-support")]
fn validate_desktop_test_profile_root(storage_root: &Path) -> rusqlite::Result<PathBuf> {
    let invalid_path = || rusqlite::Error::InvalidPath(storage_root.to_path_buf());
    if storage_root.file_name() != Some(std::ffi::OsStr::new("storage")) {
        return Err(invalid_path());
    }
    let profile_root = storage_root.parent().ok_or_else(invalid_path)?;
    let profile_metadata = fs::symlink_metadata(profile_root).map_err(filesystem_error)?;
    let profile_name = profile_root
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(invalid_path)?;
    let Some(profile_id) = profile_name.strip_prefix("openchat-desktop-test-") else {
        return Err(invalid_path());
    };
    if uuid::Uuid::parse_str(profile_id).is_err()
        || !profile_metadata.is_dir()
        || is_reparse_point(&profile_metadata)
    {
        return Err(invalid_path());
    }

    let temporary_root = fs::canonicalize(std::env::temp_dir()).map_err(filesystem_error)?;
    let canonical_profile_root = fs::canonicalize(profile_root).map_err(filesystem_error)?;
    if canonical_profile_root.parent() != Some(temporary_root.as_path()) {
        return Err(invalid_path());
    }

    match fs::symlink_metadata(storage_root) {
        Ok(metadata) if metadata.is_dir() && !is_reparse_point(&metadata) => {}
        Ok(_) => return Err(invalid_path()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(storage_root).map_err(filesystem_error)?;
        }
        Err(error) => return Err(filesystem_error(error)),
    }
    let canonical_storage_root = fs::canonicalize(storage_root).map_err(filesystem_error)?;
    if canonical_storage_root.parent() != Some(canonical_profile_root.as_path()) {
        return Err(invalid_path());
    }
    validate_desktop_test_storage_tree(&canonical_storage_root)?;
    Ok(canonical_profile_root)
}

#[cfg(feature = "desktop-profile-test-support")]
fn validate_desktop_test_storage_tree(storage_root: &Path) -> rusqlite::Result<()> {
    let invalid_path = || rusqlite::Error::InvalidPath(storage_root.to_path_buf());
    let mut directories = vec![storage_root.to_path_buf()];
    let mut inspected_entries = 0;
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory).map_err(filesystem_error)? {
            let path = entry.map_err(filesystem_error)?.path();
            let metadata = fs::symlink_metadata(&path).map_err(filesystem_error)?;
            inspected_entries += 1;
            if inspected_entries > MAXIMUM_DESKTOP_TEST_PROFILE_ENTRIES
                || is_reparse_point(&metadata)
            {
                return Err(invalid_path());
            }
            if metadata.is_dir() {
                directories.push(path);
            } else if !metadata.is_file() || is_hard_link(&path, &metadata) {
                return Err(rusqlite::Error::InvalidPath(path));
            }
        }
    }
    Ok(())
}

#[cfg(feature = "desktop-profile-test-support")]
fn read_desktop_test_key(path: &Path) -> rusqlite::Result<Option<DatabaseKey>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(filesystem_error(error)),
    };
    if !metadata.is_file()
        || is_reparse_point(&metadata)
        || metadata.len() != DATABASE_KEY_BYTES as u64
    {
        return Err(rusqlite::Error::InvalidPath(path.to_path_buf()));
    }
    let mut file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(filesystem_error)?;
    let mut bytes = Zeroizing::new([0; DATABASE_KEY_BYTES]);
    file.read_exact(&mut *bytes).map_err(filesystem_error)?;
    let mut extra = [0; 1];
    if file.read(&mut extra).map_err(filesystem_error)? != 0 {
        return Err(rusqlite::Error::InvalidPath(path.to_path_buf()));
    }
    DatabaseKey::from_bytes(&bytes[..]).map(Some)
}

#[cfg(feature = "desktop-profile-test-support")]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

#[cfg(feature = "desktop-profile-test-support")]
fn is_hard_link(path: &Path, _metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::{mem::MaybeUninit, os::windows::io::AsRawHandle};
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
        };

        let Ok(file) = File::open(path) else {
            return true;
        };
        let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
        // SAFETY: `file` owns a live handle, and `information` points to writable storage for the documented output structure.
        let succeeded =
            unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) };
        if succeeded == 0 {
            return true;
        }
        // SAFETY: The API returned success and initialized the complete output structure.
        let information = unsafe { information.assume_init() };
        information.nNumberOfLinks > 1
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        _metadata.nlink() > 1
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = path;
        let _ = _metadata;
        false
    }
}

const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";

struct MigrationPaths {
    encrypted_partial: PathBuf,
    plaintext_source: PathBuf,
    journal: PathBuf,
}

fn migration_paths(database_path: &Path) -> MigrationPaths {
    MigrationPaths {
        encrypted_partial: append_suffix(database_path, ".sqlcipher-migration.partial"),
        plaintext_source: append_suffix(database_path, ".plaintext-migration-source"),
        journal: append_suffix(database_path, ".sqlcipher-migration"),
    }
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn write_migration_journal(path: &Path) -> rusqlite::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(filesystem_error)?;
    file.write_all(MIGRATION_JOURNAL_CONTENT)
        .map_err(filesystem_error)?;
    file.sync_all().map_err(filesystem_error)
}

fn is_regular_file(path: &Path) -> rusqlite::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(true),
        Ok(_) => Err(rusqlite::Error::InvalidPath(path.to_path_buf())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(filesystem_error(error)),
    }
}

struct CreatedFile {
    path: PathBuf,
    active: bool,
}

impl CreatedFile {
    fn create(path: &Path) -> rusqlite::Result<Self> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(filesystem_error)?;
        Ok(Self {
            path: path.to_path_buf(),
            active: true,
        })
    }

    fn disarm(&mut self) {
        self.active = false;
    }

    fn remove(&mut self) -> rusqlite::Result<()> {
        if self.active {
            remove_if_present(&self.path)?;
            self.active = false;
        }
        Ok(())
    }
}

impl Drop for CreatedFile {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn sidecar_paths(database_path: &Path) -> [PathBuf; 2] {
    [
        sidecar_path(database_path, "-wal"),
        sidecar_path(database_path, "-shm"),
    ]
}

fn sidecar_path(database_path: &Path, suffix: &str) -> PathBuf {
    let mut path = database_path.as_os_str().to_os_string();
    path.push(suffix);
    PathBuf::from(path)
}

fn rename_if_present(source: &Path, destination: &Path) -> rusqlite::Result<()> {
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(filesystem_error(error)),
    }
}

fn remove_if_present(path: &Path) -> rusqlite::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(filesystem_error(error)),
    }
}

fn filesystem_error(error: io::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}

#[cfg(windows)]
pub(super) fn read_stored_database_key() -> rusqlite::Result<Option<DatabaseKey>> {
    use keyring::{Entry, Error as KeyringError};

    let entry = Entry::new("OpenChat.LocalStorage.DatabaseKey", "profile-v1")
        .map_err(|_| encryption_error())?;
    match entry.get_secret() {
        Ok(bytes) => {
            let bytes = Zeroizing::new(bytes);
            DatabaseKey::from_bytes(&bytes).map(Some)
        }
        Err(KeyringError::NoEntry) => Ok(None),
        Err(_) => Err(encryption_error()),
    }
}

#[cfg(windows)]
pub(super) fn store_database_key(key: &DatabaseKey) -> rusqlite::Result<()> {
    use keyring::Entry;

    let entry = Entry::new("OpenChat.LocalStorage.DatabaseKey", "profile-v1")
        .map_err(|_| encryption_error())?;
    entry
        .set_secret(key.as_bytes())
        .map_err(|_| encryption_error())?;
    match read_stored_database_key()? {
        Some(stored) if stored.as_bytes() == key.as_bytes() => Ok(()),
        _ => Err(encryption_error()),
    }
}

fn encryption_error() -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
        Some("The encrypted local database could not be opened safely.".to_owned()),
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use rusqlite::{Connection, OpenFlags};
    use uuid::Uuid;

    use super::DatabaseKey;

    #[test]
    fn keyed_database_does_not_have_a_plaintext_sqlite_header() {
        let directory = TestDirectory::new();
        let path = directory.0.join("profile.sqlite3");
        let key = DatabaseKey::isolated_test_key();
        let connection = key
            .open(
                &path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
            )
            .expect("open encrypted database");
        connection
            .execute_batch("CREATE TABLE probe (value TEXT NOT NULL);")
            .expect("create encrypted schema");
        drop(connection);

        let header = fs::read(&path).expect("read database header");
        assert_ne!(&header[..16], b"SQLite format 3\0");
        assert!(header.len() > 16);
    }

    #[test]
    fn keyed_database_rejects_an_incorrect_key_before_schema_use() {
        let directory = TestDirectory::new();
        let path = directory.0.join("profile.sqlite3");
        let key = DatabaseKey::isolated_test_key();
        let connection = key
            .open(
                &path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
            )
            .expect("create encrypted database");
        connection
            .execute_batch("CREATE TABLE probe (value TEXT NOT NULL);")
            .expect("create schema");
        drop(connection);

        let incorrect_key = DatabaseKey::from_bytes(&[0x5A; 32]).expect("make incorrect key");
        let error = incorrect_key
            .open(&path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .expect_err("reject incorrect key");

        assert!(matches!(error, rusqlite::Error::SqliteFailure(_, _)));
        let reopened = key
            .open(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("original key still opens the source");
        assert_eq!(
            reopened
                .query_row("SELECT count(*) FROM sqlite_master", [], |row| row
                    .get::<_, i64>(0))
                .expect("read original schema"),
            1
        );
    }

    #[cfg(feature = "desktop-profile-test-support")]
    #[test]
    fn desktop_test_profile_reuses_its_key_for_an_encrypted_database() {
        let profile = DesktopTestProfile::new();
        let storage_root = profile.0.join("storage");
        let key = super::read_or_create_desktop_test_key(&storage_root)
            .expect("create a key in the isolated desktop test profile");
        let database_path = storage_root.join("profile.sqlite3");
        let connection = key
            .open(
                &database_path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
            )
            .expect("create encrypted desktop test database");
        connection
            .execute_batch("CREATE TABLE probe (value TEXT NOT NULL);")
            .expect("create test schema");
        drop(connection);

        let reopened_key = super::read_or_create_desktop_test_key(&storage_root)
            .expect("reuse the desktop test profile key");
        let reopened = reopened_key
            .open(&database_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("reopen encrypted database after key reload");
        assert_eq!(
            reopened
                .query_row("SELECT count(*) FROM sqlite_master", [], |row| row
                    .get::<_, i64>(0))
                .expect("read encrypted schema"),
            1
        );
        assert_ne!(
            &fs::read(&database_path).expect("read encrypted database")[..16],
            b"SQLite format 3\0"
        );
    }

    #[cfg(feature = "desktop-profile-test-support")]
    #[test]
    fn desktop_test_profile_rejects_roots_outside_the_uuid_temporary_directory() {
        let directory = TestDirectory::new();
        let result = super::read_or_create_desktop_test_key(&directory.0.join("storage"));

        assert!(matches!(result, Err(rusqlite::Error::InvalidPath(_))));
        assert!(!directory.0.join("database-test-key.bin").exists());
    }

    #[cfg(feature = "desktop-profile-test-support")]
    #[test]
    fn desktop_test_profile_rejects_malformed_persisted_keys() {
        let profile = DesktopTestProfile::new();
        let storage_root = profile.0.join("storage");
        fs::create_dir(&storage_root).expect("create test storage root");
        fs::write(profile.0.join("database-test-key.bin"), [0xA5; 31])
            .expect("write malformed key fixture");

        let result = super::read_or_create_desktop_test_key(&storage_root);
        assert!(matches!(result, Err(rusqlite::Error::InvalidPath(_))));
    }

    #[cfg(feature = "desktop-profile-test-support")]
    #[test]
    fn desktop_test_profile_rejects_hard_linked_storage_files() {
        let profile = DesktopTestProfile::new();
        let outside = TestDirectory::new();
        let storage_root = profile.0.join("storage");
        let database_directory = storage_root.join("db");
        fs::create_dir_all(&database_directory).expect("create test database directory");
        let external_file = outside.0.join("external.sqlite3");
        fs::write(&external_file, b"temporary sentinel").expect("write temporary sentinel");
        fs::hard_link(&external_file, database_directory.join("openchat.sqlite3"))
            .expect("create hard-linked database fixture");

        let result = super::read_or_create_desktop_test_key(&storage_root);

        assert!(matches!(result, Err(rusqlite::Error::InvalidPath(_))));
        assert!(!profile.0.join("database-test-key.bin").exists());
    }

    #[test]
    fn plaintext_database_is_exported_to_an_encrypted_database_before_replacement() {
        let directory = TestDirectory::new();
        let path = directory.0.join("profile.sqlite3");
        let connection = Connection::open(&path).expect("create plaintext legacy database");
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 CREATE TABLE probe (value TEXT NOT NULL);
                 INSERT INTO probe VALUES ('preserved from WAL');",
            )
            .expect("write plaintext legacy data");
        drop(connection);

        let key = DatabaseKey::isolated_test_key();
        let (encrypted, migrated) = key
            .open_or_migrate(&path)
            .expect("migrate plaintext legacy database");

        assert!(migrated);
        assert_eq!(
            encrypted
                .query_row("SELECT value FROM probe", [], |row| row.get::<_, String>(0))
                .expect("read migrated value"),
            "preserved from WAL"
        );
        let header = fs::read(&path).expect("read encrypted database");
        assert_ne!(&header[..16], b"SQLite format 3\0");
        assert!(
            fs::read_dir(&directory.0)
                .expect("read isolated directory")
                .all(|entry| !entry
                    .expect("read directory entry")
                    .file_name()
                    .to_string_lossy()
                    .contains(".plaintext"))
        );
        super::super::database_guard::verify_integrity(&encrypted)
            .expect("verify encrypted database");
    }

    #[test]
    fn interrupted_plaintext_migration_is_recoverable_only_with_a_valid_source_and_journal() {
        let directory = TestDirectory::new();
        let database_path = directory.0.join("profile.sqlite3");
        let migration = super::migration_paths(&database_path);
        let source = Connection::open(&migration.plaintext_source)
            .expect("create plaintext migration source");
        source
            .execute_batch("CREATE TABLE probe (value TEXT NOT NULL);")
            .expect("write plaintext migration source");
        drop(source);
        fs::write(&migration.journal, super::MIGRATION_JOURNAL_CONTENT)
            .expect("write migration journal");

        assert!(
            DatabaseKey::has_recoverable_plaintext_migration(&database_path)
                .expect("recognize interrupted plaintext migration")
        );

        fs::write(&migration.journal, b"invalid").expect("corrupt migration journal");
        assert!(DatabaseKey::has_recoverable_plaintext_migration(&database_path).is_err());
    }

    #[test]
    fn encrypted_database_can_be_exported_under_a_different_key() {
        let directory = TestDirectory::new();
        let source_path = directory.0.join("source.sqlite3");
        let destination_path = directory.0.join("destination.sqlite3");
        let source_key = DatabaseKey::isolated_test_key();
        let destination_key = DatabaseKey::from_bytes(&[0x3C; 32]).expect("create destination key");
        let source = source_key
            .open(
                &source_path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
            )
            .expect("create encrypted source");
        source
            .execute_batch(
                "CREATE TABLE probe (value TEXT NOT NULL); INSERT INTO probe VALUES ('preserved');",
            )
            .expect("write source data");
        source
            .pragma_update(None, "user_version", 11)
            .expect("set source schema version");
        drop(source);

        source_key
            .export_encrypted_database(&source_path, &destination_path, &destination_key)
            .expect("export under archive key");

        let destination = destination_key
            .open(&destination_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("open exported database with destination key");
        assert_eq!(
            destination
                .query_row("SELECT value FROM probe", [], |row| row.get::<_, String>(0))
                .expect("read exported value"),
            "preserved"
        );
        assert_eq!(
            destination
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .expect("read exported schema version"),
            11
        );
        assert!(
            source_key
                .open(&destination_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .is_err()
        );
    }

    #[test]
    fn sqlcipher_build_exposes_an_encryption_runtime() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        DatabaseKey::isolated_test_key()
            .apply(&connection)
            .expect("key in-memory database");
        let version = connection
            .query_row("PRAGMA cipher_version", [], |row| row.get::<_, String>(0))
            .expect("read SQLCipher version");
        assert!(!version.trim().is_empty());
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("openchat-sqlcipher-{}", Uuid::new_v4()));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(feature = "desktop-profile-test-support")]
    struct DesktopTestProfile(PathBuf);

    #[cfg(feature = "desktop-profile-test-support")]
    impl DesktopTestProfile {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("openchat-desktop-test-{}", Uuid::new_v4()));
            fs::create_dir(&path).expect("create isolated desktop test profile");
            Self(path)
        }
    }

    #[cfg(feature = "desktop-profile-test-support")]
    impl Drop for DesktopTestProfile {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated desktop test profile");
        }
    }
}
