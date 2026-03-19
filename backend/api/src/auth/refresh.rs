// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Generate an opaque refresh token (a random UUID v4 string).
pub fn generate_refresh_token() -> String {
    Uuid::new_v4().to_string()
}

/// Hash a refresh token with SHA-256 and return the hex-encoded digest.
/// The hashed form is what gets stored in the database.
pub fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic() {
        let token = "some-fixed-token-value";
        let hash1 = hash_refresh_token(token);
        let hash2 = hash_refresh_token(token);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn different_tokens_different_hashes() {
        let hash1 = hash_refresh_token("token-aaa");
        let hash2 = hash_refresh_token("token-bbb");
        assert_ne!(hash1, hash2);
    }
}
