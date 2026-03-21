// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub exp: i64,
    pub iat: i64,
}

/// Create a signed JWT access token for the given user.
pub fn encode_access_token(
    user_id: Uuid,
    secret: &str,
    expiry_seconds: u64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = chrono::Utc::now().timestamp();
    let claims = Claims {
        sub: user_id,
        exp: now + expiry_seconds as i64,
        iat: now,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

/// Decode and validate a JWT access token, returning the claims.
pub fn decode_access_token(
    token: &str,
    secret: &str,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.validate_exp = true;
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;
    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_encode_decode() {
        let user_id = Uuid::new_v4();
        let secret = "test-secret-at-least-32-bytes-long";
        let token = encode_access_token(user_id, secret, 3600).unwrap();
        let claims = decode_access_token(&token, secret).unwrap();
        assert_eq!(claims.sub, user_id);
    }

    #[test]
    fn wrong_secret_fails() {
        let user_id = Uuid::new_v4();
        let token = encode_access_token(user_id, "correct-secret", 3600).unwrap();
        let result = decode_access_token(&token, "wrong-secret");
        assert!(result.is_err());
    }

    #[test]
    fn expired_token_fails() {
        let user_id = Uuid::new_v4();
        let secret = "test-secret-at-least-32-bytes-long";
        // Create a token that expired 3600 seconds ago (past the default leeway)
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            sub: user_id,
            exp: now - 3600,
            iat: now - 7200,
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap();
        let result = decode_access_token(&token, secret);
        assert!(result.is_err());
    }
}
