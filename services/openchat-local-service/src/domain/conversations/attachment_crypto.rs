//! Profile-key-bound encryption for message attachment files.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
};

use ring::{
    aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
    hkdf::{HKDF_SHA256, Salt},
    rand::{SecureRandom, SystemRandom},
};
use zeroize::Zeroizing;

use crate::{conversation_limits::MAXIMUM_ATTACHMENT_IMAGE_BYTES, storage::DatabaseKey};

const ENVELOPE_FAMILY: &[u8] = b"\0OpenChatAttachment/";
const ENVELOPE_MAGIC: &[u8] = b"\0OpenChatAttachment/v1\0";
const AUTH_TAG_LENGTH: usize = 16;
const ENVELOPE_OVERHEAD: usize = ENVELOPE_MAGIC.len() + NONCE_LEN + AUTH_TAG_LENGTH;
const ATTACHMENT_KEY_SALT: &[u8] = b"OpenChat profile attachment key salt v1";
const ATTACHMENT_KEY_INFO: &[u8] = b"OpenChat attachment content encryption key v1";

pub(crate) struct AttachmentCipher {
    key: Zeroizing<[u8; 32]>,
}

impl AttachmentCipher {
    pub(crate) fn from_profile_key(profile_key: &DatabaseKey) -> io::Result<Self> {
        let salt = Salt::new(HKDF_SHA256, ATTACHMENT_KEY_SALT);
        let pseudorandom_key = salt.extract(profile_key.as_bytes());
        let output = pseudorandom_key
            .expand(&[ATTACHMENT_KEY_INFO], HKDF_SHA256)
            .map_err(|_| invalid_envelope())?;
        let mut key = Zeroizing::new([0; 32]);
        output.fill(&mut *key).map_err(|_| invalid_envelope())?;
        Ok(Self { key })
    }

    pub(crate) fn seal(&self, associated_data: &[u8], plaintext: &[u8]) -> io::Result<Vec<u8>> {
        let key = LessSafeKey::new(
            UnboundKey::new(&AES_256_GCM, &self.key[..]).map_err(|_| invalid_envelope())?,
        );
        let mut nonce_bytes = [0; NONCE_LEN];
        SystemRandom::new()
            .fill(&mut nonce_bytes)
            .map_err(|_| invalid_envelope())?;
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);
        let mut ciphertext = Zeroizing::new(plaintext.to_vec());
        key.seal_in_place_append_tag(nonce, Aad::from(associated_data), &mut *ciphertext)
            .map_err(|_| invalid_envelope())?;

        let mut envelope = Vec::with_capacity(ENVELOPE_OVERHEAD + plaintext.len());
        envelope.extend_from_slice(ENVELOPE_MAGIC);
        envelope.extend_from_slice(&nonce_bytes);
        envelope.extend_from_slice(&ciphertext);
        Ok(envelope)
    }

    pub(crate) fn open(
        &self,
        associated_data: &[u8],
        envelope: &[u8],
    ) -> io::Result<Zeroizing<Vec<u8>>> {
        if !is_current_envelope(envelope)? || envelope.len() < ENVELOPE_OVERHEAD {
            return Err(invalid_envelope());
        }
        let nonce_start = ENVELOPE_MAGIC.len();
        let nonce_end = nonce_start + NONCE_LEN;
        let nonce = Nonce::try_assume_unique_for_key(&envelope[nonce_start..nonce_end])
            .map_err(|_| invalid_envelope())?;
        let mut plaintext = Zeroizing::new(envelope[nonce_end..].to_vec());
        let key = LessSafeKey::new(
            UnboundKey::new(&AES_256_GCM, &self.key[..]).map_err(|_| invalid_envelope())?,
        );
        let plaintext_length = key
            .open_in_place(nonce, Aad::from(associated_data), &mut plaintext)
            .map_err(|_| invalid_envelope())?
            .len();
        plaintext.truncate(plaintext_length);
        Ok(plaintext)
    }

    pub(crate) fn read_file(
        &self,
        path: &Path,
        associated_data: &[u8],
        maximum_plaintext_bytes: usize,
    ) -> io::Result<Zeroizing<Vec<u8>>> {
        let metadata = checked_attachment_file(path)?;
        let maximum_envelope_bytes = maximum_plaintext_bytes
            .checked_add(ENVELOPE_OVERHEAD)
            .ok_or_else(invalid_envelope)?;
        if metadata.len() > maximum_envelope_bytes as u64 {
            return Err(invalid_envelope());
        }
        let mut file = File::open(path)?;
        if has_multiple_links(&file)? {
            return Err(invalid_envelope());
        }
        let mut envelope = Zeroizing::new(Vec::with_capacity(metadata.len() as usize));
        Read::by_ref(&mut file)
            .take(maximum_envelope_bytes as u64 + 1)
            .read_to_end(&mut envelope)?;
        if envelope.len() as u64 != metadata.len() {
            return Err(invalid_envelope());
        }
        self.open(associated_data, &envelope)
    }

    pub(crate) fn has_encrypted_file_shape(
        path: &Path,
        expected_plaintext_bytes: usize,
    ) -> io::Result<bool> {
        let metadata = checked_attachment_file(path)?;
        let expected_envelope_bytes = expected_plaintext_bytes
            .checked_add(ENVELOPE_OVERHEAD)
            .ok_or_else(invalid_envelope)?;
        if metadata.len() != expected_envelope_bytes as u64 {
            return Ok(false);
        }
        let mut file = File::open(path)?;
        if has_multiple_links(&file)? {
            return Ok(false);
        }
        let mut magic = [0; ENVELOPE_MAGIC.len()];
        file.read_exact(&mut magic)?;
        Ok(&magic == ENVELOPE_MAGIC)
    }

    pub(crate) fn write_new_file(
        &self,
        path: &Path,
        associated_data: &[u8],
        plaintext: &[u8],
    ) -> io::Result<()> {
        let envelope = self.seal(associated_data, plaintext)?;
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(&envelope)?;
        file.sync_all()
    }
}

pub(crate) fn attachment_associated_data(relative_path: &Path) -> io::Result<Vec<u8>> {
    let mut components = Vec::new();
    for component in relative_path.components() {
        let Component::Normal(value) = component else {
            return Err(invalid_envelope());
        };
        let value = value.to_str().ok_or_else(invalid_envelope)?;
        if value.is_empty() || value.contains('/') || value.contains('\\') || !value.is_ascii() {
            return Err(invalid_envelope());
        }
        components.push(value);
    }
    if !(2..=3).contains(&components.len())
        || !components.last().is_some_and(|name| {
            name.strip_suffix(".data")
                .is_some_and(is_attachment_identifier)
        })
        || components[..components.len() - 1]
            .iter()
            .any(|component| !is_attachment_directory_name(component))
    {
        return Err(invalid_envelope());
    }

    let path = components.join("/");
    let mut associated_data = Vec::with_capacity(40 + path.len());
    associated_data.extend_from_slice(b"OpenChat attachment path v1\0attachments/");
    associated_data.extend_from_slice(path.as_bytes());
    Ok(associated_data)
}

pub(crate) fn migrate_attachment_tree(
    attachments_root: &Path,
    profile_key: &DatabaseKey,
) -> io::Result<usize> {
    let root_metadata = match fs::symlink_metadata(attachments_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(invalid_envelope());
    }
    let root = fs::canonicalize(attachments_root)?;
    let cipher = AttachmentCipher::from_profile_key(profile_key)?;
    let mut migrated = 0;
    migrate_directory(&root, &root, &cipher, &mut migrated, 0)?;
    Ok(migrated)
}

fn migrate_directory(
    root: &Path,
    directory: &Path,
    cipher: &AttachmentCipher,
    migrated: &mut usize,
    depth: usize,
) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(invalid_envelope());
        }
        if file_type.is_dir() {
            if depth >= 2 || !is_attachment_directory_name(&entry.file_name().to_string_lossy()) {
                return Err(invalid_envelope());
            }
            migrate_directory(root, &path, cipher, migrated, depth + 1)?;
            continue;
        }
        if !file_type.is_file() {
            return Err(invalid_envelope());
        }
        if !entry.file_name().to_string_lossy().ends_with(".data") {
            continue;
        }
        let relative_path = path.strip_prefix(root).map_err(|_| invalid_envelope())?;
        let associated_data = attachment_associated_data(relative_path)?;
        if migrate_file(cipher, &path, &associated_data)? {
            *migrated = migrated.saturating_add(1);
        }
    }
    Ok(())
}

fn migrate_file(
    cipher: &AttachmentCipher,
    path: &Path,
    associated_data: &[u8],
) -> io::Result<bool> {
    let metadata = checked_attachment_file(path)?;
    let maximum_envelope_bytes = MAXIMUM_ATTACHMENT_IMAGE_BYTES
        .checked_add(ENVELOPE_OVERHEAD)
        .ok_or_else(invalid_envelope)?;
    if metadata.len() > maximum_envelope_bytes as u64 {
        return Err(invalid_envelope());
    }
    let mut source = File::open(path)?;
    if has_multiple_links(&source)? {
        return Err(invalid_envelope());
    }
    let mut prefix = [0; ENVELOPE_MAGIC.len()];
    let prefix_length = source.read(&mut prefix)?;
    let prefix = &prefix[..prefix_length];
    if prefix.starts_with(ENVELOPE_MAGIC) {
        if metadata.len() < ENVELOPE_OVERHEAD as u64 {
            return Err(invalid_envelope());
        }
        return Ok(false);
    }
    if prefix.starts_with(ENVELOPE_FAMILY) {
        return Err(invalid_envelope());
    }
    source.seek(SeekFrom::Start(0))?;
    let mut contents = Zeroizing::new(Vec::with_capacity(metadata.len() as usize));
    Read::by_ref(&mut source)
        .take(maximum_envelope_bytes as u64 + 1)
        .read_to_end(&mut contents)?;
    if contents.len() as u64 != metadata.len() {
        return Err(invalid_envelope());
    }
    drop(source);
    if is_current_envelope(&contents)? {
        if contents.len() < ENVELOPE_OVERHEAD {
            return Err(invalid_envelope());
        }
        return Ok(false);
    }

    let temporary = migration_temporary_path(path)?;
    if let Some(temporary_contents) = read_optional_migration_temporary(&temporary)? {
        if let Ok(plaintext) = cipher.open(associated_data, &temporary_contents)
            && plaintext.as_slice() == contents.as_slice()
        {
            replace_file_atomically(&temporary, path)?;
            sync_parent_directory(path)?;
            return Ok(true);
        }
        fs::remove_file(&temporary)?;
    }

    let encrypted = cipher.seal(associated_data, &contents)?;
    let mut temporary_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    temporary_file.write_all(&encrypted)?;
    temporary_file.sync_all()?;
    drop(temporary_file);

    let verified = cipher.read_file(&temporary, associated_data, MAXIMUM_ATTACHMENT_IMAGE_BYTES)?;
    if verified.as_slice() != contents.as_slice() {
        return Err(invalid_envelope());
    }
    replace_file_atomically(&temporary, path)?;
    sync_parent_directory(path)?;
    Ok(true)
}

fn has_multiple_links(file: &File) -> io::Result<bool> {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{
            Foundation::HANDLE,
            Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle},
        };

        let mut information = BY_HANDLE_FILE_INFORMATION::default();
        // SAFETY: `file` owns a live Windows handle and `information` is a writable output value.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut information) }
            == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(information.nNumberOfLinks > 1)
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(file.metadata()?.nlink() > 1)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = file;
        Ok(true)
    }
}

fn checked_attachment_file(path: &Path) -> io::Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid_envelope());
    }
    Ok(metadata)
}

fn migration_temporary_path(path: &Path) -> io::Result<PathBuf> {
    let file_name = path.file_name().ok_or_else(invalid_envelope)?;
    let mut temporary_name = std::ffi::OsString::from(".");
    temporary_name.push(file_name);
    temporary_name.push(".encrypting");
    Ok(path.with_file_name(temporary_name))
}

fn read_optional_migration_temporary(path: &Path) -> io::Result<Option<Zeroizing<Vec<u8>>>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid_envelope());
    }
    if metadata.len() > (MAXIMUM_ATTACHMENT_IMAGE_BYTES + ENVELOPE_OVERHEAD) as u64 {
        return Err(invalid_envelope());
    }
    let mut file = File::open(path)?;
    if has_multiple_links(&file)? {
        return Err(invalid_envelope());
    }
    let mut contents = Zeroizing::new(Vec::with_capacity(metadata.len() as usize));
    Read::by_ref(&mut file)
        .take((MAXIMUM_ATTACHMENT_IMAGE_BYTES + ENVELOPE_OVERHEAD) as u64 + 1)
        .read_to_end(&mut contents)?;
    if contents.len() as u64 != metadata.len() {
        return Err(invalid_envelope());
    }
    Ok(Some(contents))
}

fn replace_file_atomically(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        };

        let source = source
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let destination = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        // SAFETY: Both path buffers are NUL-terminated and remain live for the duration of the call.
        if unsafe {
            MoveFileExW(
                source.as_ptr(),
                destination.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        fs::rename(source, destination)
    }
}

fn sync_parent_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        let parent = path.parent().ok_or_else(invalid_envelope)?;
        File::open(parent)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

fn is_current_envelope(bytes: &[u8]) -> io::Result<bool> {
    if bytes.starts_with(ENVELOPE_MAGIC) {
        return Ok(true);
    }
    if bytes.starts_with(ENVELOPE_FAMILY) {
        return Err(invalid_envelope());
    }
    Ok(false)
}

fn is_attachment_directory_name(value: &str) -> bool {
    is_attachment_identifier(value)
        || value
            .strip_prefix(".stage-")
            .is_some_and(is_attachment_identifier)
}

fn is_attachment_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn invalid_envelope() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "attachment file format or authentication is invalid",
    )
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::Instant,
    };

    use super::{AttachmentCipher, attachment_associated_data, migrate_attachment_tree};
    use crate::storage::DatabaseKey;
    use zeroize::Zeroizing;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-attachment-crypto-{}",
                uuid::Uuid::new_v4().simple()
            ));
            fs::create_dir_all(&path).expect("create attachment test root");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn profile_key() -> DatabaseKey {
        DatabaseKey::from_bytes(&[0x5a; 32]).expect("create test profile key")
    }

    fn test_path(root: &Path) -> PathBuf {
        root.join("conversation-1")
            .join("message-1")
            .join("attachment-1.data")
    }

    #[test]
    fn attachment_envelopes_round_trip_and_bind_the_file_path() {
        let cipher = AttachmentCipher::from_profile_key(&profile_key()).expect("derive key");
        let context =
            attachment_associated_data(Path::new("conversation-1/message-1/attachment-1.data"))
                .expect("build associated data");
        let encrypted = cipher
            .seal(&context, b"private attachment bytes")
            .expect("encrypt attachment");

        assert!(
            !encrypted
                .windows(b"private attachment bytes".len())
                .any(|window| window == b"private attachment bytes")
        );
        assert_eq!(
            cipher
                .open(&context, &encrypted)
                .expect("decrypt attachment")
                .as_slice(),
            b"private attachment bytes"
        );
        assert!(
            cipher
                .open(
                    &attachment_associated_data(Path::new(
                        "conversation-2/message-1/attachment-1.data",
                    ))
                    .expect("build changed associated data"),
                    &encrypted,
                )
                .is_err()
        );
    }

    #[test]
    fn attachment_envelopes_reject_tampering_and_a_different_profile_key() {
        let cipher = AttachmentCipher::from_profile_key(&profile_key()).expect("derive key");
        let context =
            attachment_associated_data(Path::new("conversation-1/message-1/attachment-1.data"))
                .expect("build associated data");
        let mut encrypted = cipher.seal(&context, b"attachment bytes").expect("encrypt");
        let last = encrypted.len() - 1;
        encrypted[last] ^= 1;
        assert!(cipher.open(&context, &encrypted).is_err());

        let other_key = DatabaseKey::from_bytes(&[0xa5; 32]).expect("create alternate key");
        let other_cipher = AttachmentCipher::from_profile_key(&other_key).expect("derive key");
        assert!(other_cipher.open(&context, &encrypted).is_err());
    }

    #[test]
    fn attachment_migration_recovers_a_verified_temporary_and_is_idempotent() {
        let directory = TestDirectory::new();
        let root = directory.0.join("attachments");
        let path = test_path(&root);
        fs::create_dir_all(path.parent().expect("message directory")).expect("create directory");
        let plaintext = b"legacy attachment content";
        fs::write(&path, plaintext).expect("write legacy attachment");

        let key = profile_key();
        let cipher = AttachmentCipher::from_profile_key(&key).expect("derive key");
        let context =
            attachment_associated_data(Path::new("conversation-1/message-1/attachment-1.data"))
                .expect("build associated data");
        let temporary = super::migration_temporary_path(&path).expect("temporary path");
        fs::write(
            &temporary,
            cipher.seal(&context, plaintext).expect("encrypt temporary"),
        )
        .expect("write verified temporary");

        assert_eq!(
            migrate_attachment_tree(&root, &key).expect("recover migration"),
            1
        );
        assert!(
            !fs::read(&path)
                .expect("read migrated attachment")
                .windows(plaintext.len())
                .any(|window| window == plaintext)
        );
        assert_eq!(
            migrate_attachment_tree(&root, &key).expect("repeat migration"),
            0
        );
        assert_eq!(
            cipher
                .read_file(&path, &context, plaintext.len())
                .expect("read encrypted attachment")
                .as_slice(),
            plaintext
        );
    }

    #[test]
    fn interrupted_attachment_migration_replaces_a_partial_temp_without_losing_the_source() {
        let directory = TestDirectory::new();
        let root = directory.0.join("attachments");
        let path = test_path(&root);
        fs::create_dir_all(path.parent().expect("message directory")).expect("create directory");
        let plaintext = b"legacy attachment content";
        fs::write(&path, plaintext).expect("write legacy attachment");
        let temporary = super::migration_temporary_path(&path).expect("temporary path");
        fs::write(&temporary, b"partial temporary envelope").expect("write interrupted temp");

        assert!(migrate_attachment_tree(&root, &profile_key()).is_ok());
        let cipher = AttachmentCipher::from_profile_key(&profile_key()).expect("derive key");
        let context =
            attachment_associated_data(Path::new("conversation-1/message-1/attachment-1.data"))
                .expect("build associated data");
        assert_eq!(
            cipher
                .read_file(&path, &context, plaintext.len())
                .expect("read migrated file")
                .as_slice(),
            plaintext
        );
    }

    #[test]
    fn unsupported_attachment_envelope_keeps_the_original_file_unchanged() {
        let directory = TestDirectory::new();
        let root = directory.0.join("attachments");
        let path = test_path(&root);
        fs::create_dir_all(path.parent().expect("message directory"))
            .expect("create attachment directory");
        let unsupported = b"\0OpenChatAttachment/v2\0unsupported";
        fs::write(&path, unsupported).expect("write unsupported attachment envelope");

        assert!(migrate_attachment_tree(&root, &profile_key()).is_err());
        assert_eq!(
            fs::read(path).expect("read preserved unsupported attachment"),
            unsupported
        );
    }

    #[test]
    #[ignore = "manual synthetic benchmark for one-time attachment migration and warm startup scan"]
    fn benchmark_attachment_migration_and_warm_scan() {
        const RUNS: usize = 5;
        const ATTACHMENT_COUNT: usize = 100;
        const ATTACHMENT_BYTES: usize = 128 * 1024;

        let payload = Zeroizing::new(vec![0x5a; ATTACHMENT_BYTES]);
        let mut migration_times = Vec::with_capacity(RUNS);
        let mut warm_scan_times = Vec::with_capacity(RUNS);
        for _ in 0..RUNS {
            let directory = TestDirectory::new();
            let root = directory.0.join("attachments");
            for index in 0..ATTACHMENT_COUNT {
                let path = root
                    .join(format!("conversation-{}/message-{index}", index / 10))
                    .join(format!("attachment-{index}.data"));
                fs::create_dir_all(path.parent().expect("message directory"))
                    .expect("create synthetic attachment directory");
                fs::write(path, payload.as_slice()).expect("write synthetic legacy attachment");
            }

            let key = profile_key();
            let migration_started = Instant::now();
            assert_eq!(
                migrate_attachment_tree(&root, &key).expect("migrate synthetic attachments"),
                ATTACHMENT_COUNT
            );
            migration_times.push(migration_started.elapsed());

            let warm_scan_started = Instant::now();
            assert_eq!(
                migrate_attachment_tree(&root, &key).expect("scan migrated attachments"),
                0
            );
            warm_scan_times.push(warm_scan_started.elapsed());
            drop(directory);
        }

        migration_times.sort_unstable();
        warm_scan_times.sort_unstable();
        println!(
            "Synthetic migration: {ATTACHMENT_COUNT} x {ATTACHMENT_BYTES} B, {RUNS} runs; initial p50={:.2} ms, max={:.2} ms; warm scan p50={:.2} ms, max={:.2} ms",
            migration_times[RUNS / 2].as_secs_f64() * 1000.0,
            migration_times[RUNS - 1].as_secs_f64() * 1000.0,
            warm_scan_times[RUNS / 2].as_secs_f64() * 1000.0,
            warm_scan_times[RUNS - 1].as_secs_f64() * 1000.0,
        );
    }
}
