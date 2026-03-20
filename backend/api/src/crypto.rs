// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Cryptographic operations.
//!
//! AES-256-GCM encryption and decryption for integration tokens.
//! All token encrypt/decrypt in the application goes through this module.

use aes_gcm::aead::{Aead, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Key, KeyInit, Nonce};

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
/// Returns hex-encoded `nonce || ciphertext || tag`.
pub fn encrypt(plaintext: &str, key: &[u8; 32]) -> Result<String, CryptoError> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|_| CryptoError::EncryptionFailed)?;

    let mut combined = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    combined.extend_from_slice(&nonce);
    combined.extend_from_slice(&ciphertext);

    Ok(hex::encode(combined))
}

/// Decrypt a hex-encoded `nonce || ciphertext || tag` string with AES-256-GCM.
pub fn decrypt(ciphertext_hex: &str, key: &[u8; 32]) -> Result<String, CryptoError> {
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

    #[test]
    fn round_trip() {
        let key = test_key();
        let plaintext = "my-secret-token-12345";
        let encrypted = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn round_trip_empty_string() {
        let key = test_key();
        let encrypted = encrypt("", &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();
        assert_eq!(decrypted, "");
    }

    #[test]
    fn round_trip_unicode() {
        let key = test_key();
        let plaintext = "token-with-unicode-\u{1f600}-\u{00e9}";
        let encrypted = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();
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
        assert_eq!(decrypt(&c1, &key).unwrap(), plaintext);
        assert_eq!(decrypt(&c2, &key).unwrap(), plaintext);
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let key = test_key();
        let wrong_key = parse_encryption_key(
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        )
        .unwrap();
        let encrypted = encrypt("secret", &key).unwrap();
        let result = decrypt(&encrypted, &wrong_key);
        assert!(matches!(result, Err(CryptoError::DecryptionFailed)));
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = test_key();
        let encrypted = encrypt("secret", &key).unwrap();
        // Flip a character in the ciphertext portion (after the 24-char hex nonce).
        let mut chars: Vec<char> = encrypted.chars().collect();
        let idx = 30; // well into the ciphertext area
        chars[idx] = if chars[idx] == 'a' { 'b' } else { 'a' };
        let tampered: String = chars.into_iter().collect();
        let result = decrypt(&tampered, &key);
        assert!(matches!(result, Err(CryptoError::DecryptionFailed)));
    }

    #[test]
    fn invalid_hex_fails() {
        let key = test_key();
        let result = decrypt("not-valid-hex!!!", &key);
        assert!(matches!(result, Err(CryptoError::InvalidCiphertext(_))));
    }

    #[test]
    fn too_short_ciphertext_fails() {
        let key = test_key();
        // 12 bytes nonce = 24 hex chars, but no ciphertext at all.
        let result = decrypt("aabbccddaabbccddaabbccdd", &key);
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
}
