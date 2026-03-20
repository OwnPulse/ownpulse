// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Generate an opaque refresh token (a random UUID v4 string).
pub fn generate_refresh_token() -> String {
    Uuid::new_v4().to_string()
}

/// Hash a refresh token with HMAC-SHA256 keyed on the provided secret and
/// return the hex-encoded MAC.  The hashed form is what gets stored in the
/// database.
pub fn hash_refresh_token(token: &str, secret: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(token.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic() {
        let token = "some-fixed-token-value";
        let secret = "test-secret";
        let hash1 = hash_refresh_token(token, secret);
        let hash2 = hash_refresh_token(token, secret);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn different_tokens_different_hashes() {
        let secret = "test-secret";
        let hash1 = hash_refresh_token("token-aaa", secret);
        let hash2 = hash_refresh_token("token-bbb", secret);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn different_secrets_different_hashes() {
        let token = "same-token";
        let hash1 = hash_refresh_token(token, "secret-a");
        let hash2 = hash_refresh_token(token, "secret-b");
        assert_ne!(hash1, hash2);
    }
}
