//! Stores per-account, per-device sync keys in the operating system keyring.

use std::io;

use keyring::{Entry, Error as KeyringError};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::sync_crypto::SyncKey;

const KEYRING_SERVICE: &str = "OpenChat.Sync.DeviceKey";
const RECORD_MAGIC: &[u8; 8] = b"OCSYNC\0\x01";
const RECORD_BYTES: usize = RECORD_MAGIC.len() + 4 + 32;

/// Loads one stored key epoch for an enrolled account and device.
///
/// Returns `Ok(None)` when no key was enrolled for that exact scope. It never
/// creates a replacement key while loading.
pub fn load_sync_key(account_id: Uuid, device_id: Uuid, epoch: u32) -> io::Result<Option<SyncKey>> {
    if epoch == 0 {
        return Err(invalid_key_record());
    }
    let entry = entry(account_id, device_id, epoch)?;
    load_sync_key_from_entry(&entry, epoch)
}

fn load_sync_key_from_entry(entry: &Entry, epoch: u32) -> io::Result<Option<SyncKey>> {
    match entry.get_secret() {
        Ok(bytes) => {
            let bytes = Zeroizing::new(bytes);
            decode_key(&bytes, epoch).map(Some)
        }
        Err(KeyringError::NoEntry) => Ok(None),
        Err(_) => Err(keyring_unavailable()),
    }
}

/// Stores and verifies a sync key in a namespace separate from local database
/// encryption and provider credentials.
pub fn store_sync_key(account_id: Uuid, device_id: Uuid, key: &SyncKey) -> io::Result<()> {
    let entry = entry(account_id, device_id, key.epoch())?;
    store_sync_key_in_entry(&entry, key)
}

fn store_sync_key_in_entry(entry: &Entry, key: &SyncKey) -> io::Result<()> {
    let record = encode_key(key);
    entry
        .set_secret(&record)
        .map_err(|_| keyring_unavailable())?;
    let stored = Zeroizing::new(entry.get_secret().map_err(|_| keyring_unavailable())?);
    decode_key(&stored, key.epoch())?;
    if stored.as_slice() != record.as_slice() {
        return Err(invalid_key_record());
    }
    Ok(())
}

fn entry(account_id: Uuid, device_id: Uuid, epoch: u32) -> io::Result<Entry> {
    Entry::new(
        KEYRING_SERVICE,
        &format!("v1/{account_id}/{device_id}/{epoch}"),
    )
    .map_err(|_| keyring_unavailable())
}

fn encode_key(key: &SyncKey) -> Zeroizing<Vec<u8>> {
    let mut record = Zeroizing::new(Vec::with_capacity(RECORD_BYTES));
    record.extend_from_slice(RECORD_MAGIC);
    record.extend_from_slice(&key.epoch().to_be_bytes());
    record.extend_from_slice(key.as_bytes());
    record
}

fn decode_key(record: &[u8], expected_epoch: u32) -> io::Result<SyncKey> {
    if record.len() != RECORD_BYTES || !record.starts_with(RECORD_MAGIC) {
        return Err(invalid_key_record());
    }
    let epoch_start = RECORD_MAGIC.len();
    let epoch_end = epoch_start + 4;
    let epoch = u32::from_be_bytes(
        record[epoch_start..epoch_end]
            .try_into()
            .map_err(|_| invalid_key_record())?,
    );
    if epoch != expected_epoch {
        return Err(invalid_key_record());
    }
    SyncKey::from_bytes(epoch, &record[epoch_end..]).map_err(|_| invalid_key_record())
}

fn keyring_unavailable() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "sync key storage is unavailable",
    )
}

fn invalid_key_record() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid stored sync key")
}

#[cfg(test)]
mod tests {
    use std::{
        any::Any,
        io,
        sync::{Arc, Mutex},
    };

    use keyring::{Entry, Error as KeyringError, credential::CredentialApi};
    use uuid::Uuid;
    use zeroize::Zeroizing;

    use crate::sync_crypto::SyncKey;

    use super::{
        RECORD_BYTES, RECORD_MAGIC, decode_key, encode_key, load_sync_key_from_entry,
        store_sync_key_in_entry,
    };

    #[test]
    fn stored_record_is_versioned_and_round_trips_the_key_epoch() {
        let key = SyncKey::from_bytes(7, &[0x2A; 32]).expect("valid sync key");
        let record = encode_key(&key);

        assert_eq!(record.len(), RECORD_BYTES);
        assert!(record.starts_with(RECORD_MAGIC));
        let restored = decode_key(&record, 7).expect("decode stored key");
        assert_eq!(restored.epoch(), 7);
        assert_eq!(restored.as_bytes(), key.as_bytes());
    }

    #[test]
    fn invalid_version_length_and_epoch_are_rejected() {
        let key = SyncKey::from_bytes(3, &[0x55; 32]).expect("valid sync key");
        let record = encode_key(&key);
        assert!(decode_key(&record, 4).is_err());

        let mut invalid = Zeroizing::new(record.to_vec());
        invalid[0] ^= 1;
        assert!(decode_key(&invalid, 3).is_err());
        assert!(decode_key(&invalid[..invalid.len() - 1], 3).is_err());
    }

    #[test]
    fn account_device_and_epoch_are_isolated_in_the_reference() {
        let account = Uuid::new_v4();
        let device = Uuid::new_v4();
        let other_device = Uuid::new_v4();
        assert_ne!(
            reference(account, device, 1),
            reference(account, other_device, 1)
        );
        assert_ne!(reference(account, device, 1), reference(account, device, 2));
    }

    #[test]
    fn keyring_store_and_load_reuse_the_separate_sync_key() {
        let value = Arc::new(Mutex::new(None));
        let entry = Entry::new_with_credential(Box::new(TestCredential(Arc::clone(&value))));
        let key = SyncKey::from_bytes(9, &[0x31; 32]).expect("valid sync key");

        store_sync_key_in_entry(&entry, &key).expect("store sync key");
        let loaded = load_sync_key_from_entry(&entry, 9)
            .expect("load sync key")
            .expect("stored key exists");
        assert_eq!(loaded.epoch(), 9);
        assert_eq!(loaded.as_bytes(), key.as_bytes());
    }

    #[test]
    fn absent_key_is_not_created_during_load() {
        let value = Arc::new(Mutex::new(None));
        let entry = Entry::new_with_credential(Box::new(TestCredential(Arc::clone(&value))));

        assert!(
            load_sync_key_from_entry(&entry, 2)
                .expect("missing key is valid state")
                .is_none()
        );
        assert!(value.lock().expect("read mock storage").is_none());
    }

    fn reference(account: Uuid, device: Uuid, epoch: u32) -> String {
        format!("v1/{account}/{device}/{epoch}")
    }

    struct TestCredential(Arc<Mutex<Option<Vec<u8>>>>);

    impl CredentialApi for TestCredential {
        fn set_secret(&self, secret: &[u8]) -> keyring::Result<()> {
            *self.0.lock().map_err(|_| unavailable())? = Some(secret.to_vec());
            Ok(())
        }

        fn get_secret(&self) -> keyring::Result<Vec<u8>> {
            self.0
                .lock()
                .map_err(|_| unavailable())?
                .clone()
                .ok_or(KeyringError::NoEntry)
        }

        fn delete_credential(&self) -> keyring::Result<()> {
            self.0
                .lock()
                .map_err(|_| unavailable())?
                .take()
                .map(|_| ())
                .ok_or(KeyringError::NoEntry)
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    fn unavailable() -> KeyringError {
        KeyringError::NoStorageAccess(Box::new(io::Error::other("mock keyring unavailable")))
    }
}
