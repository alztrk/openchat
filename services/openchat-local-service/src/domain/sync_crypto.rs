//! Versioned content encryption for end-to-end sync envelopes.
//!
//! The sync root key is a separate secret from the local profile/database and
//! attachment keys. Each envelope gets a fresh salt and a derived AES-GCM key.

use std::io;

use ring::{
    aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
    hkdf::{HKDF_SHA256, Salt},
    rand::{SecureRandom, SystemRandom},
};
use zeroize::Zeroizing;

const ENVELOPE_FAMILY: &[u8] = b"\0OpenChatSync/";
const ENVELOPE_MAGIC: &[u8] = b"\0OpenChatSync/v1\0";
const KEY_SALT_LENGTH: usize = 16;
const AUTH_TAG_LENGTH: usize = 16;
const KEY_INFO: &[u8] = b"OpenChat sync envelope content key v1\0";
const MAXIMUM_SYNC_PAYLOAD_BYTES: usize = 32 * 1024 * 1024;
const ENVELOPE_OVERHEAD: usize =
    ENVELOPE_MAGIC.len() + 4 + KEY_SALT_LENGTH + NONCE_LEN + AUTH_TAG_LENGTH;

/// A versioned root key dedicated to sync content encryption.
pub struct SyncKey {
    epoch: u32,
    bytes: Zeroizing<[u8; 32]>,
}

impl SyncKey {
    /// Generate a random sync root key for a nonzero key epoch.
    pub fn generate(epoch: u32) -> io::Result<Self> {
        if epoch == 0 {
            return Err(invalid_envelope());
        }
        let mut bytes = Zeroizing::new([0; 32]);
        SystemRandom::new()
            .fill(&mut *bytes)
            .map_err(|_| invalid_envelope())?;
        Ok(Self { epoch, bytes })
    }

    /// Restore a sync root key from its secure platform storage.
    ///
    /// The supplied bytes must never be a profile database or attachment key.
    pub fn from_bytes(epoch: u32, bytes: &[u8]) -> io::Result<Self> {
        if epoch == 0 {
            return Err(invalid_envelope());
        }
        let bytes: [u8; 32] = bytes.try_into().map_err(|_| invalid_envelope())?;
        Ok(Self {
            epoch,
            bytes: Zeroizing::new(bytes),
        })
    }

    /// Return this key's nonzero rotation epoch.
    pub fn epoch(&self) -> u32 {
        self.epoch
    }
}

/// Encrypts and decrypts bounded v1 sync envelopes for one key epoch.
pub struct SyncCipher {
    key: SyncKey,
}

impl SyncCipher {
    /// Create a cipher that owns the supplied sync root key.
    pub fn new(key: SyncKey) -> Self {
        Self { key }
    }

    /// Encrypt one payload and authenticate caller-supplied canonical associated data.
    pub fn seal(&self, associated_data: &[u8], plaintext: &[u8]) -> io::Result<Vec<u8>> {
        if plaintext.len() > MAXIMUM_SYNC_PAYLOAD_BYTES {
            return Err(invalid_envelope());
        }

        let mut salt_bytes = [0; KEY_SALT_LENGTH];
        let mut nonce_bytes = [0; NONCE_LEN];
        let random = SystemRandom::new();
        random
            .fill(&mut salt_bytes)
            .and_then(|()| random.fill(&mut nonce_bytes))
            .map_err(|_| invalid_envelope())?;
        let key = self.derive_content_key(&salt_bytes)?;
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);
        let mut ciphertext = Zeroizing::new(plaintext.to_vec());
        key.seal_in_place_append_tag(nonce, Aad::from(associated_data), &mut *ciphertext)
            .map_err(|_| invalid_envelope())?;

        let mut envelope = Vec::with_capacity(ENVELOPE_OVERHEAD + plaintext.len());
        envelope.extend_from_slice(ENVELOPE_MAGIC);
        envelope.extend_from_slice(&self.key.epoch.to_be_bytes());
        envelope.extend_from_slice(&salt_bytes);
        envelope.extend_from_slice(&nonce_bytes);
        envelope.extend_from_slice(&ciphertext);
        Ok(envelope)
    }

    /// Authenticate and decrypt an envelope using its exact associated data and key epoch.
    pub fn open(&self, associated_data: &[u8], envelope: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
        if envelope.len() < ENVELOPE_OVERHEAD
            || envelope.len() > MAXIMUM_SYNC_PAYLOAD_BYTES + ENVELOPE_OVERHEAD
        {
            return Err(invalid_envelope());
        }
        if !is_current_envelope(envelope) {
            return Err(invalid_envelope());
        }

        let epoch_start = ENVELOPE_MAGIC.len();
        let epoch_end = epoch_start + 4;
        let epoch_bytes: [u8; 4] = envelope[epoch_start..epoch_end]
            .try_into()
            .map_err(|_| invalid_envelope())?;
        if u32::from_be_bytes(epoch_bytes) != self.key.epoch {
            return Err(invalid_envelope());
        }

        let salt_start = epoch_end;
        let salt_end = salt_start + KEY_SALT_LENGTH;
        let nonce_start = salt_end;
        let nonce_end = nonce_start + NONCE_LEN;
        let key = self.derive_content_key(&envelope[salt_start..salt_end])?;
        let nonce = Nonce::try_assume_unique_for_key(&envelope[nonce_start..nonce_end])
            .map_err(|_| invalid_envelope())?;
        let mut plaintext = Zeroizing::new(envelope[nonce_end..].to_vec());
        let plaintext_length = key
            .open_in_place(nonce, Aad::from(associated_data), &mut plaintext)
            .map_err(|_| invalid_envelope())?
            .len();
        plaintext.truncate(plaintext_length);
        Ok(plaintext)
    }

    fn derive_content_key(&self, salt_bytes: &[u8]) -> io::Result<LessSafeKey> {
        let salt = Salt::new(HKDF_SHA256, salt_bytes);
        let pseudorandom_key = salt.extract(&self.key.bytes[..]);
        let epoch = self.key.epoch.to_be_bytes();
        let info = [KEY_INFO, &epoch[..]];
        let output = pseudorandom_key
            .expand(&info, HKDF_SHA256)
            .map_err(|_| invalid_envelope())?;
        let mut key_bytes = Zeroizing::new([0; 32]);
        output
            .fill(&mut *key_bytes)
            .map_err(|_| invalid_envelope())?;
        let unbound =
            UnboundKey::new(&AES_256_GCM, &key_bytes[..]).map_err(|_| invalid_envelope())?;
        Ok(LessSafeKey::new(unbound))
    }
}

fn is_current_envelope(envelope: &[u8]) -> bool {
    envelope.starts_with(ENVELOPE_FAMILY) && envelope.starts_with(ENVELOPE_MAGIC)
}

fn invalid_envelope() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid sync envelope")
}

#[cfg(test)]
mod tests {
    use super::{ENVELOPE_FAMILY, ENVELOPE_MAGIC, SyncCipher, SyncKey};

    fn key(epoch: u32) -> SyncKey {
        SyncKey::from_bytes(epoch, &[7; 32]).expect("valid sync key")
    }

    #[test]
    fn envelope_round_trips_with_key_epoch_and_associated_data() {
        let cipher = SyncCipher::new(key(1));
        let associated_data = b"account-1/conversation/conversation-1/revision-2";
        let envelope = cipher
            .seal(associated_data, b"encrypted conversation snapshot")
            .expect("seal sync data");

        assert!(envelope.starts_with(ENVELOPE_MAGIC));
        assert_eq!(
            &envelope[ENVELOPE_MAGIC.len()..ENVELOPE_MAGIC.len() + 4],
            &1_u32.to_be_bytes()
        );
        assert_eq!(
            cipher
                .open(associated_data, &envelope)
                .expect("open sync envelope")
                .as_slice(),
            b"encrypted conversation snapshot"
        );
    }

    #[test]
    fn each_envelope_uses_fresh_salt_and_nonce() {
        let cipher = SyncCipher::new(key(1));
        let first = cipher.seal(b"record", b"same payload").expect("seal first");
        let second = cipher
            .seal(b"record", b"same payload")
            .expect("seal second");

        assert_ne!(first, second);
        assert_ne!(
            &first[ENVELOPE_MAGIC.len() + 4..ENVELOPE_MAGIC.len() + 4 + 16],
            &second[ENVELOPE_MAGIC.len() + 4..ENVELOPE_MAGIC.len() + 4 + 16]
        );
    }

    #[test]
    fn envelope_rejects_wrong_associated_data_key_epoch_and_tampering() {
        let cipher = SyncCipher::new(key(1));
        let envelope = cipher
            .seal(b"record-a", b"private data")
            .expect("seal data");
        assert!(cipher.open(b"record-b", &envelope).is_err());
        assert!(
            SyncCipher::new(key(2))
                .open(b"record-a", &envelope)
                .is_err()
        );

        let mut tampered = envelope;
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert!(cipher.open(b"record-a", &tampered).is_err());
    }

    #[test]
    fn rejects_unknown_version_and_invalid_key_epoch() {
        let cipher = SyncCipher::new(key(1));
        let mut envelope = cipher.seal(b"record", b"payload").expect("seal data");
        envelope[ENVELOPE_FAMILY.len() + 1] = b'2';
        assert!(cipher.open(b"record", &envelope).is_err());
        assert!(SyncKey::generate(0).is_err());
        assert!(SyncKey::from_bytes(1, &[0; 31]).is_err());
    }
}
