// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Cryptographic operations.
//!
//! AES-256-GCM encryption and decryption for integration tokens.
//! All token encrypt/decrypt in the application goes through this module.
//!
//! ## Key rotation
//!
//! Encrypted values are prefixed with a version tag: `v1:<hex>`.
//! Legacy values stored without a prefix are also supported — they are tried
//! against the current key first, then against `previous_key` if provided.
//!
//! To rotate keys:
//! 1. Set `ENCRYPTION_KEY` to the new key.
//! 2. Set `ENCRYPTION_KEY_PREVIOUS` to the old key.
//! 3. Re-encrypt stored values opportunistically (on read/write).
//! 4. Once all values carry the `v1:` prefix, `ENCRYPTION_KEY_PREVIOUS` can
//!    be unset.

use aes_gcm::aead::{Aead, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Key, KeyInit, Nonce};

/// Version tag prepended to every newly-encrypted value.
const CURRENT_KEY_VERSION: &str = "v1";

/// Nonce size for AES-256-GCM (96 bits = 12 bytes).
const NONCE_SIZE: usize = 12;

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("invalid encryption key: expected 64 hex characters (32 bytes)")]
    InvalidKey,
    #[error("encryption failed")]
    EncryptionFailed,
    #[error("decryption failed")]
    DecryptionFailed,
    #[error("invalid ciphertext: {0}")]
    InvalidCiphertext(String),
}

/// Parse a 32-byte encryption key from a 64-character hex string.
pub fn parse_encryption_key(hex_str: &str) -> Result<[u8; 32], CryptoError> {
    let bytes = hex::decode(hex_str).map_err(|_| CryptoError::InvalidKey)?;
    let key: [u8; 32] = bytes.try_into().map_err(|_| CryptoError::InvalidKey)?;
    Ok(key)
}

/// Encrypt `plaintext` with AES-256-GCM using a random 96-bit nonce.
///
/// Returns a versioned string: `v1:<hex-encoded nonce || ciphertext || tag>`.
pub fn encrypt(plaintext: &str, key: &[u8; 32]) -> Result<String, CryptoError> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|_| CryptoError::EncryptionFailed)?;

    let mut combined = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    combined.extend_from_slice(&nonce);
    combined.extend_from_slice(&ciphertext);

    Ok(format!("{}:{}", CURRENT_KEY_VERSION, hex::encode(combined)))
}

/// Decrypt a ciphertext string, handling both versioned (`v1:...`) and legacy
/// (unversioned hex) formats.
///
/// For legacy values, the current key is tried first. If that fails and
/// `previous_key` is provided, it is tried as a fallback to support key
/// rotation in progress.
pub fn decrypt(
    ciphertext: &str,
    current_key: &[u8; 32],
    previous_key: Option<&[u8; 32]>,
) -> Result<String, CryptoError> {
    if let Some(rest) = ciphertext.strip_prefix("v1:") {
        decrypt_raw(rest, current_key)
    } else {
        // Legacy unversioned — try current key, fall back to previous.
        match decrypt_raw(ciphertext, current_key) {
            Ok(pt) => Ok(pt),
            Err(_) if previous_key.is_some() => {
                // previous_key.is_some() is checked in the guard above.
                decrypt_raw(ciphertext, previous_key.unwrap())
            }
            Err(e) => Err(e),
        }
    }
}

/// Low-level decrypt: expects a bare hex-encoded `nonce || ciphertext || tag`.
/// Not exposed publicly — callers should use [`decrypt`].
fn decrypt_raw(ciphertext_hex: &str, key: &[u8; 32]) -> Result<String, CryptoError> {
    let combined = hex::decode(ciphertext_hex)
        .map_err(|_| CryptoError::InvalidCiphertext("invalid hex encoding".to_string()))?;

    if combined.len() < NONCE_SIZE + 1 {
        return Err(CryptoError::InvalidCiphertext(
            "ciphertext too short".to_string(),
        ));
    }

    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_SIZE);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| CryptoError::DecryptionFailed)?;

    String::from_utf8(plaintext)
        .map_err(|_| CryptoError::InvalidCiphertext("decrypted data is not valid UTF-8".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        parse_encryption_key(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .unwrap()
    }

    fn alt_key() -> [u8; 32] {
        parse_encryption_key(
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        )
        .unwrap()
    }

    // --- existing tests (updated to new decrypt signature) ---

    #[test]
    fn round_trip() {
        let key = test_key();
        let plaintext = "my-secret-token-12345";
        let encrypted = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key, None).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn round_trip_empty_string() {
        let key = test_key();
        let encrypted = encrypt("", &key).unwrap();
        let decrypted = decrypt(&encrypted, &key, None).unwrap();
        assert_eq!(decrypted, "");
    }

    #[test]
    fn round_trip_unicode() {
        let key = test_key();
        let plaintext = "token-with-unicode-\u{1f600}-\u{00e9}";
        let encrypted = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key, None).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn different_nonces_produce_different_ciphertexts() {
        let key = test_key();
        let plaintext = "same-plaintext";
        let c1 = encrypt(plaintext, &key).unwrap();
        let c2 = encrypt(plaintext, &key).unwrap();
        assert_ne!(c1, c2, "two encryptions of the same plaintext must differ");
        // Both must still decrypt correctly.
        assert_eq!(decrypt(&c1, &key, None).unwrap(), plaintext);
        assert_eq!(decrypt(&c2, &key, None).unwrap(), plaintext);
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let key = test_key();
        let wrong_key = alt_key();
        let encrypted = encrypt("secret", &key).unwrap();
        let result = decrypt(&encrypted, &wrong_key, None);
        assert!(matches!(result, Err(CryptoError::DecryptionFailed)));
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = test_key();
        let encrypted = encrypt("secret", &key).unwrap();
        // Flip a character in the ciphertext portion (after the "v1:" prefix
        // and the 24-char hex nonce, so well into the ciphertext area).
        let mut chars: Vec<char> = encrypted.chars().collect();
        let idx = 30; // well into the ciphertext area
        chars[idx] = if chars[idx] == 'a' { 'b' } else { 'a' };
        let tampered: String = chars.into_iter().collect();
        let result = decrypt(&tampered, &key, None);
        assert!(matches!(result, Err(CryptoError::DecryptionFailed)));
    }

    #[test]
    fn invalid_hex_fails() {
        let key = test_key();
        // Bare (no version prefix) non-hex string.
        let result = decrypt("not-valid-hex!!!", &key, None);
        assert!(matches!(result, Err(CryptoError::InvalidCiphertext(_))));
    }

    #[test]
    fn too_short_ciphertext_fails() {
        let key = test_key();
        // 12 bytes nonce = 24 hex chars, but no ciphertext at all.
        let result = decrypt("aabbccddaabbccddaabbccdd", &key, None);
        assert!(matches!(result, Err(CryptoError::InvalidCiphertext(_))));
    }

    #[test]
    fn parse_key_valid() {
        let key = parse_encryption_key(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );
        assert!(key.is_ok());
        assert_eq!(key.unwrap().len(), 32);
    }

    #[test]
    fn parse_key_too_short() {
        let result = parse_encryption_key("0123456789abcdef");
        assert!(matches!(result, Err(CryptoError::InvalidKey)));
    }

    #[test]
    fn parse_key_invalid_hex() {
        let result = parse_encryption_key(
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        );
        assert!(matches!(result, Err(CryptoError::InvalidKey)));
    }

    // --- new key-rotation tests ---

    #[test]
    fn versioned_encrypt_decrypt_roundtrip() {
        let key = test_key();
        let plaintext = "rotation-test-token";
        let encrypted = encrypt(plaintext, &key).unwrap();

        // Output must carry the version prefix.
        assert!(
            encrypted.starts_with("v1:"),
            "expected v1: prefix, got: {encrypted}"
        );

        let decrypted = decrypt(&encrypted, &key, None).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn legacy_ciphertext_decrypts_with_current_key() {
        // Simulate a value stored before versioning was introduced: bare hex,
        // no "v1:" prefix. Produced by calling decrypt_raw directly.
        let key = test_key();
        let plaintext = "legacy-token-value";

        // Build a raw (unversioned) ciphertext manually.
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ct = cipher.encrypt(&nonce, plaintext.as_bytes()).unwrap();
        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ct);
        let legacy_hex = hex::encode(combined);

        // No "v1:" prefix — should decrypt fine with current key.
        let decrypted = decrypt(&legacy_hex, &key, None).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn legacy_ciphertext_falls_back_to_previous_key() {
        let old_key = test_key();
        let new_key = alt_key();
        let plaintext = "still-readable-after-rotation";

        // Produce a legacy (unversioned) ciphertext with the OLD key.
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&old_key));
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ct = cipher.encrypt(&nonce, plaintext.as_bytes()).unwrap();
        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ct);
        let legacy_hex = hex::encode(combined);

        // current=new_key, previous=old_key — should fall back and succeed.
        let decrypted = decrypt(&legacy_hex, &new_key, Some(&old_key)).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn wrong_keys_fail() {
        let key_a = test_key();
        let key_b = alt_key();
        let key_c = parse_encryption_key(
            "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        )
        .unwrap();

        // Encrypt with key_a, try decrypting with key_b (current) + key_c (previous).
        let encrypted = encrypt("secret", &key_a).unwrap();
        let result = decrypt(&encrypted, &key_b, Some(&key_c));
        assert!(
            matches!(result, Err(CryptoError::DecryptionFailed)),
            "expected DecryptionFailed, got: {result:?}"
        );
    }
}
