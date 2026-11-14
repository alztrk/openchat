# Sync encryption envelope

This document defines the first local cryptographic primitive for hosted sync. It does not define an account service, pairing protocol, relay API, or production deployment.

## Key hierarchy

- A sync root key is 32 random bytes, distinct from the SQLCipher profile key, attachment key, provider credentials, and tool secrets.
- A monotonically increasing nonzero key epoch identifies the root key version. A device must select the matching root key before opening an envelope; it must never retry with another epoch.
- Each envelope carries a fresh 16-byte salt. HKDF-SHA-256 derives a 32-byte content key from the sync root key, the salt, a fixed v1 domain string, and the key epoch.
- The derived content key encrypts one payload only and is held in zeroizing memory. The root key must be persisted only by the platform secure-key store once device-key storage is implemented.

## Envelope v1

The binary envelope is the concatenation of:

1. `\0OpenChatSync/v1\0` magic and version marker;
2. the key epoch as an unsigned 32-bit big-endian integer;
3. the per-envelope 16-byte HKDF salt;
4. a fresh 12-byte AES-GCM nonce;
5. AES-256-GCM ciphertext followed by its 16-byte authentication tag.

The caller supplies canonical associated data to both seal and open. The authenticated data is not copied into the envelope. Future transport code must bind account, record type, record identifier, and revision consistently on both ends. Those routing fields may be visible to the relay; metadata minimization and the synchronized data scope remain open protocol decisions.

The service rejects unknown versions, wrong key epochs, malformed/truncated envelopes, invalid authentication tags, and payloads over 32 MiB. It does not fall back to plaintext or another format.

## Security boundary

AES-GCM provides authenticated encryption with associated data, and HKDF follows the extract-and-expand key derivation construction. This primitive only protects payload bytes at rest or in transit when the caller provides the right sync root key and canonical associated data. It does not authenticate accounts or devices, prevent replay, resolve conflicts, guarantee forward secrecy, recover lost device keys, authenticate a relay, or prove that a hosted service does not retain plaintext. Those requirements remain open in issues #159 and #160.

References: [RFC 5869](https://www.rfc-editor.org/rfc/rfc5869), [NIST SP 800-38D](https://csrc.nist.gov/pubs/sp/800/38/d/final), [ring AES-256-GCM](https://docs.rs/ring/latest/ring/aead/static.AES_256_GCM.html).
