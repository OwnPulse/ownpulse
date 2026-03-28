// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
    pub iss: String,
}

/// Create a signed JWT access token for the given user.
pub fn encode_access_token(
    user_id: Uuid,
    role: &str,
    secret: &str,
    issuer: &str,
    expiry_seconds: u64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = chrono::Utc::now().timestamp();
    let claims = Claims {
        sub: user_id,
        role: role.to_string(),
        exp: now + expiry_seconds as i64,
        iat: now,
        iss: issuer.to_string(),
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
    issuer: &str,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.validate_exp = true;
    validation.set_issuer(&[issuer]);
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
        let issuer = "http://localhost:5173";
        let token = encode_access_token(user_id, "user", secret, issuer, 3600).unwrap();
        let claims = decode_access_token(&token, secret, issuer).unwrap();
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.role, "user");
        assert_eq!(claims.iss, "http://localhost:5173");
    }

    #[test]
    fn wrong_secret_fails() {
        let user_id = Uuid::new_v4();
        let issuer = "http://localhost:5173";
        let token = encode_access_token(user_id, "user", "correct-secret", issuer, 3600).unwrap();
        let result = decode_access_token(&token, "wrong-secret", issuer);
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
            role: "user".to_string(),
            exp: now - 3600,
            iat: now - 7200,
            iss: "http://localhost:5173".to_string(),
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap();
        let result = decode_access_token(&token, secret, "http://localhost:5173");
        assert!(result.is_err());
    }

    #[test]
    fn alg_none_rejected() {
        // Manually construct a token with {"alg":"none","typ":"JWT"} header
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
        let now = chrono::Utc::now().timestamp();
        let user_id = Uuid::new_v4();
        let payload = URL_SAFE_NO_PAD.encode(format!(
            r#"{{"sub":"{}","role":"admin","exp":{},"iat":{},"iss":"http://localhost:5173"}}"#,
            user_id,
            now + 3600,
            now
        ));
        let token = format!("{}.{}.", header, payload); // empty signature

        let result = decode_access_token(&token, "any-secret", "http://localhost:5173");
        assert!(result.is_err(), "alg:none token must be rejected");
    }

    #[test]
    fn wrong_algorithm_rejected() {
        // A token signed with HS384 must be rejected because only HS256 is allowed
        use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};

        let user_id = Uuid::new_v4();
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            sub: user_id,
            role: "user".to_string(),
            exp: now + 3600,
            iat: now,
            iss: "http://localhost:5173".to_string(),
        };

        // Encode with HS384 (not HS256)
        let token = encode(
            &Header::new(Algorithm::HS384),
            &claims,
            &EncodingKey::from_secret(b"test-secret"),
        )
        .unwrap();

        let result = decode_access_token(&token, "test-secret", "http://localhost:5173");
        assert!(
            result.is_err(),
            "HS384 token must be rejected when HS256 is expected"
        );
    }

    #[test]
    fn role_admin_roundtrips() {
        let user_id = Uuid::new_v4();
        let secret = "test-secret-at-least-32-bytes-long";
        let issuer = "http://localhost:5173";
        let token = encode_access_token(user_id, "admin", secret, issuer, 3600).unwrap();
        let claims = decode_access_token(&token, secret, issuer).unwrap();
        assert_eq!(claims.role, "admin");
    }

    #[test]
    fn wrong_issuer_rejected() {
        let user_id = Uuid::new_v4();
        let secret = "test-secret-at-least-32-bytes-long";
        let token = encode_access_token(
            user_id,
            "user",
            secret,
            "https://app.staging.ownpulse.health",
            3600,
        )
        .unwrap();
        let result = decode_access_token(&token, secret, "https://app.ownpulse.health");
        assert!(
            result.is_err(),
            "token minted for staging must be rejected on production"
        );
    }
}
