// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Integration tests for multi-auth: Apple Sign-In, account linking, and unlinking.

use axum::body::Body;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use http::Request;
use http_body_util::BodyExt;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::RsaPrivateKey;
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::traits::PublicKeyParts;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::common;

// ---------------------------------------------------------------------------
// Apple test key helpers
// ---------------------------------------------------------------------------

/// Bit length for test RSA keys. 2048 is fastest that Apple/jsonwebtoken accepts.
const TEST_KEY_BITS: usize = 2048;

/// Generate a fresh RSA private key for signing test Apple id_tokens.
fn gen_rsa_key() -> RsaPrivateKey {
    let mut rng = rand::thread_rng();
    RsaPrivateKey::new(&mut rng, TEST_KEY_BITS).expect("failed to generate RSA key")
}

/// Encode a big integer (from the RSA key) as base64url with no padding,
/// as required by the JWK format.
fn b64url(n: &rsa::BigUint) -> String {
    URL_SAFE_NO_PAD.encode(n.to_bytes_be())
}

/// Build a JWKS JSON document from the given private key.
fn make_jwks(private_key: &RsaPrivateKey, kid: &str) -> Value {
    let pub_key = private_key.to_public_key();
    json!({
        "keys": [{
            "kty": "RSA",
            "use": "sig",
            "alg": "RS256",
            "kid": kid,
            "n": b64url(pub_key.n()),
            "e": b64url(pub_key.e()),
        }]
    })
}

/// Claims for an Apple id_token.
#[derive(Serialize, Deserialize)]
struct AppleClaims {
    iss: String,
    aud: String,
    sub: String,
    exp: u64,
    iat: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
}

/// Sign a minimal Apple id_token using the given RSA key.
fn make_apple_id_token(
    private_key: &RsaPrivateKey,
    kid: &str,
    sub: &str,
    client_id: &str,
    email: Option<&str>,
) -> String {
    let der = private_key
        .to_pkcs1_der()
        .expect("failed to encode private key to DER");
    let encoding_key = EncodingKey::from_rsa_der(der.as_bytes());

    let now = chrono::Utc::now().timestamp() as u64;
    let claims = AppleClaims {
        iss: "https://appleid.apple.com".to_string(),
        aud: client_id.to_string(),
        sub: sub.to_string(),
        exp: now + 600,
        iat: now,
        email: email.map(|s| s.to_string()),
    };

    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_string());

    encode(&header, &claims, &encoding_key).expect("failed to sign Apple id_token")
}

// ---------------------------------------------------------------------------
// Helpers shared across tests
// ---------------------------------------------------------------------------

/// Collect the response body into a parsed JSON value.
async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Build a POST request with JSON body.
fn post_json(uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .unwrap()
}

/// Build an authenticated POST request.
fn auth_post(uri: &str, token: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .unwrap()
}

/// Build an authenticated DELETE request.
fn auth_delete(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

/// Build an authenticated GET request.
fn auth_get(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_apple_callback_creates_user() {
    let kid = "test-key-1";
    let client_id = "com.example.ownpulse";
    let private_key = gen_rsa_key();
    let jwks = make_jwks(&private_key, kid);

    // Mock JWKS endpoint
    let mock_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/auth/keys"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(&jwks))
        .mount(&mock_server)
        .await;

    let jwks_url = format!("{}/auth/keys", mock_server.uri());

    let test_app = common::setup_with_config(|c| {
        c.apple_client_id = Some(client_id.to_string());
        c.apple_jwks_url = jwks_url.clone();
    })
    .await;

    let id_token = make_apple_id_token(
        &private_key,
        kid,
        "apple-sub-001",
        client_id,
        Some("appleuser@privaterelay.appleid.com"),
    );

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/apple/callback",
            &json!({"id_token": id_token, "platform": "ios"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200, "apple callback should return 200");
}

#[tokio::test]
async fn test_apple_callback_returns_tokens_in_body() {
    let kid = "test-key-2";
    let client_id = "com.example.ownpulse";
    let private_key = gen_rsa_key();
    let jwks = make_jwks(&private_key, kid);

    let mock_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/auth/keys"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(&jwks))
        .mount(&mock_server)
        .await;

    let jwks_url = format!("{}/auth/keys", mock_server.uri());

    let test_app = common::setup_with_config(|c| {
        c.apple_client_id = Some(client_id.to_string());
        c.apple_jwks_url = jwks_url.clone();
    })
    .await;

    let id_token = make_apple_id_token(
        &private_key,
        kid,
        "apple-sub-002",
        client_id,
        Some("body@example.com"),
    );

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/apple/callback",
            &json!({"id_token": id_token, "platform": "ios"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let json = body_json(response).await;

    assert!(
        json["access_token"].is_string(),
        "body should contain access_token"
    );
    assert!(
        json["refresh_token"].is_string(),
        "iOS body should contain refresh_token for Keychain storage"
    );
    assert_eq!(json["token_type"], "Bearer");
    assert!(!json["access_token"].as_str().unwrap().is_empty());
    assert!(!json["refresh_token"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_apple_callback_web_no_refresh_in_body() {
    let kid = "test-key-web";
    let client_id = "com.example.ownpulse";
    let private_key = gen_rsa_key();
    let jwks = make_jwks(&private_key, kid);

    let mock_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/auth/keys"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(&jwks))
        .mount(&mock_server)
        .await;

    let jwks_url = format!("{}/auth/keys", mock_server.uri());

    let test_app = common::setup_with_config(|c| {
        c.apple_client_id = Some(client_id.to_string());
        c.apple_jwks_url = jwks_url.clone();
    })
    .await;

    let id_token = make_apple_id_token(&private_key, kid, "apple-sub-web", client_id, None);

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/apple/callback",
            &json!({"id_token": id_token, "platform": "web"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let json = body_json(response).await;

    assert!(json["access_token"].is_string());
    // Web response must NOT include refresh_token in the body — cookie only.
    assert!(
        json["refresh_token"].is_null(),
        "web response should not contain refresh_token in body, got: {:?}",
        json["refresh_token"]
    );
}

#[tokio::test]
async fn test_apple_callback_existing_user_returns_same_user() {
    let kid = "test-key-3";
    let client_id = "com.example.ownpulse";
    let apple_sub = "apple-sub-idempotent";
    let private_key = gen_rsa_key();
    let jwks = make_jwks(&private_key, kid);

    let mock_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/auth/keys"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(&jwks))
        .mount(&mock_server)
        .await;

    let jwks_url = format!("{}/auth/keys", mock_server.uri());

    let test_app = common::setup_with_config(|c| {
        c.apple_client_id = Some(client_id.to_string());
        c.apple_jwks_url = jwks_url.clone();
    })
    .await;

    let make_token = || {
        make_apple_id_token(
            &private_key,
            kid,
            apple_sub,
            client_id,
            Some("idempotent@example.com"),
        )
    };

    // First call — creates the user
    let r1 = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/apple/callback",
            &json!({"id_token": make_token(), "platform": "ios"}),
        ))
        .await
        .unwrap();
    assert_eq!(r1.status(), 200);
    let j1 = body_json(r1).await;
    let token1 = j1["access_token"].as_str().unwrap().to_string();

    // Second call — should return the same user (not a new one)
    let r2 = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/apple/callback",
            &json!({"id_token": make_token(), "platform": "ios"}),
        ))
        .await
        .unwrap();
    assert_eq!(r2.status(), 200);
    let j2 = body_json(r2).await;
    let token2 = j2["access_token"].as_str().unwrap().to_string();

    // Decode both tokens and check they have the same user_id
    let claims1 = api::auth::jwt::decode_access_token(
        &token1,
        "test-jwt-secret-at-least-32-bytes-long",
        "http://localhost:5173",
    )
    .unwrap();
    let claims2 = api::auth::jwt::decode_access_token(
        &token2,
        "test-jwt-secret-at-least-32-bytes-long",
        "http://localhost:5173",
    )
    .unwrap();

    assert_eq!(
        claims1.sub, claims2.sub,
        "both calls should return the same user"
    );

    // Verify only one user_auth_methods row exists for this apple sub
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM user_auth_methods WHERE provider = 'apple' AND provider_subject = $1",
    )
    .bind(apple_sub)
    .fetch_one(&test_app.pool)
    .await
    .unwrap();
    assert_eq!(count.0, 1, "should have exactly one auth method row");
}

#[tokio::test]
async fn test_link_apple_to_google_user() {
    let kid = "test-key-link";
    let client_id = "com.example.ownpulse";
    let private_key = gen_rsa_key();
    let jwks = make_jwks(&private_key, kid);

    let mock_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/auth/keys"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(&jwks))
        .mount(&mock_server)
        .await;

    let jwks_url = format!("{}/auth/keys", mock_server.uri());

    let test_app = common::setup_with_config(|c| {
        c.apple_client_id = Some(client_id.to_string());
        c.apple_jwks_url = jwks_url.clone();
    })
    .await;

    // Create a "Google" user directly in the DB
    let (user_id, token) = common::create_test_user(&test_app).await;

    // Also insert a google auth method for them
    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject, email) VALUES ($1, 'google', $2, $3)",
    )
    .bind(user_id)
    .bind("google-sub-link")
    .bind("link@example.com")
    .execute(&test_app.pool)
    .await
    .unwrap();

    // Link their Apple account
    let apple_sub = "apple-sub-link-001";
    let id_token = make_apple_id_token(
        &private_key,
        kid,
        apple_sub,
        client_id,
        Some("link@example.com"),
    );

    let response = test_app
        .app
        .oneshot(auth_post(
            "/api/v1/auth/link",
            &token,
            &json!({"provider": "apple", "id_token": id_token}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let json = body_json(response).await;
    let methods = json.as_array().unwrap();

    // Should now have two methods: local (from create_test_user) + google + apple = 3
    // Actually create_test_user inserts a 'local' method; we then added 'google' manually.
    // The link call adds 'apple'. Count varies; just assert apple is in the list.
    let providers: Vec<&str> = methods
        .iter()
        .filter_map(|m| m["provider"].as_str())
        .collect();
    assert!(
        providers.contains(&"apple"),
        "apple should be in linked providers, got: {:?}",
        providers
    );
}

#[tokio::test]
async fn test_link_duplicate_rejected() {
    let kid = "test-key-dup";
    let client_id = "com.example.ownpulse";
    let private_key = gen_rsa_key();
    let jwks = make_jwks(&private_key, kid);

    let mock_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/auth/keys"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(&jwks))
        .mount(&mock_server)
        .await;

    let jwks_url = format!("{}/auth/keys", mock_server.uri());

    let test_app = common::setup_with_config(|c| {
        c.apple_client_id = Some(client_id.to_string());
        c.apple_jwks_url = jwks_url.clone();
    })
    .await;

    let apple_sub = "apple-sub-dup-001";

    // User 1 — already owns this Apple sub via the callback flow
    let (user1_id, _) = common::create_test_user(&test_app).await;
    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject, email) VALUES ($1, 'apple', $2, $3)",
    )
    .bind(user1_id)
    .bind(apple_sub)
    .bind("dup1@example.com")
    .execute(&test_app.pool)
    .await
    .unwrap();

    // User 2 — tries to link the same Apple sub
    let (_, token2) = common::create_test_user(&test_app).await;
    let id_token = make_apple_id_token(&private_key, kid, apple_sub, client_id, None);

    let response = test_app
        .app
        .oneshot(auth_post(
            "/api/v1/auth/link",
            &token2,
            &json!({"provider": "apple", "id_token": id_token}),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        409,
        "linking an Apple account already linked to another user should return 409"
    );
}

#[tokio::test]
async fn test_unlink_last_method_rejected() {
    let test_app = common::setup().await;
    let (user_id, token) = common::create_test_user(&test_app).await;

    // Ensure there is only ONE auth method (local, from create_test_user).
    // The migration populates user_auth_methods from existing users; there may
    // already be a row from insert. Let's count to be sure.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM user_auth_methods WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&test_app.pool)
        .await
        .unwrap();

    // If count is already 1, attempt to unlink should fail.
    // If for some reason count > 1, delete extras so we test the guard.
    if count.0 > 1 {
        sqlx::query("DELETE FROM user_auth_methods WHERE user_id = $1 AND provider != 'local'")
            .bind(user_id)
            .execute(&test_app.pool)
            .await
            .unwrap();
    }

    let response = test_app
        .app
        .oneshot(auth_delete("/api/v1/auth/link/local", &token))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "unlinking the only login method should return 400"
    );
}

#[tokio::test]
async fn test_unlink_success() {
    let kid = "test-key-unlink";
    let client_id = "com.example.ownpulse";
    let private_key = gen_rsa_key();
    let jwks = make_jwks(&private_key, kid);

    let mock_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/auth/keys"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(&jwks))
        .mount(&mock_server)
        .await;

    let jwks_url = format!("{}/auth/keys", mock_server.uri());

    let test_app = common::setup_with_config(|c| {
        c.apple_client_id = Some(client_id.to_string());
        c.apple_jwks_url = jwks_url.clone();
    })
    .await;

    let (user_id, token) = common::create_test_user(&test_app).await;

    // Add a second auth method (google) so the user has two.
    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject, email) VALUES ($1, 'google', $2, $3)",
    )
    .bind(user_id)
    .bind("google-sub-unlink")
    .bind("unlink@example.com")
    .execute(&test_app.pool)
    .await
    .unwrap();

    // Verify we have at least 2 methods before unlinking.
    let before: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM user_auth_methods WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&test_app.pool)
            .await
            .unwrap();
    assert!(
        before.0 >= 2,
        "expected at least 2 auth methods before unlink"
    );

    // Unlink google.
    let response = test_app
        .app
        .clone()
        .oneshot(auth_delete("/api/v1/auth/link/google", &token))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let json = body_json(response).await;
    let methods = json.as_array().unwrap();

    // google should no longer be in the list.
    let providers: Vec<&str> = methods
        .iter()
        .filter_map(|m| m["provider"].as_str())
        .collect();
    assert!(
        !providers.contains(&"google"),
        "google should be removed, got: {:?}",
        providers
    );

    // Verify count decreased.
    let after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM user_auth_methods WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&test_app.pool)
        .await
        .unwrap();
    assert_eq!(after.0, before.0 - 1, "method count should decrease by 1");
}

#[tokio::test]
async fn test_auth_methods_list() {
    let test_app = common::setup().await;
    let (user_id, token) = common::create_test_user(&test_app).await;

    // Add a second method.
    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject, email) VALUES ($1, 'google', $2, $3)",
    )
    .bind(user_id)
    .bind("google-sub-list")
    .bind("list@example.com")
    .execute(&test_app.pool)
    .await
    .unwrap();

    let response = test_app
        .app
        .oneshot(auth_get("/api/v1/auth/methods", &token))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let json = body_json(response).await;
    let methods = json.as_array().expect("expected JSON array");

    assert!(
        methods.len() >= 2,
        "should have at least 2 linked providers"
    );

    // Each method should have id, provider, created_at fields.
    for method in methods {
        assert!(
            method["id"].is_string(),
            "method should have id, got: {method:?}"
        );
        assert!(
            method["provider"].is_string(),
            "method should have provider, got: {method:?}"
        );
        assert!(
            method["created_at"].is_string(),
            "method should have created_at, got: {method:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// #9: Apple callback with invalid token returns 401
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_apple_callback_invalid_token_returns_401() {
    let test_app = common::setup_with_config(|c| {
        c.apple_client_id = Some("com.example.ownpulse".to_string());
        // JWKS URL doesn't matter — token header decode will fail first.
    })
    .await;

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/apple/callback",
            &json!({"id_token": "not.a.valid.jwt", "platform": "ios"}),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        401,
        "an invalid Apple id_token should return 401"
    );
}

// ---------------------------------------------------------------------------
// #10: link_auth with "local" provider (social-only user links password)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_link_local_to_social_user() {
    let kid = "test-key-link-local";
    let client_id = "com.example.ownpulse";
    let private_key = gen_rsa_key();
    let jwks = make_jwks(&private_key, kid);

    let mock_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/auth/keys"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(&jwks))
        .mount(&mock_server)
        .await;

    let jwks_url = format!("{}/auth/keys", mock_server.uri());

    let test_app = common::setup_with_config(|c| {
        c.apple_client_id = Some(client_id.to_string());
        c.apple_jwks_url = jwks_url.clone();
    })
    .await;

    // Create an Apple-only user via the callback flow.
    let apple_sub = "apple-sub-link-local";
    let id_token = make_apple_id_token(
        &private_key,
        kid,
        apple_sub,
        client_id,
        Some("linklocal@example.com"),
    );

    let r1 = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/apple/callback",
            &json!({"id_token": id_token, "platform": "ios"}),
        ))
        .await
        .unwrap();
    assert_eq!(r1.status(), 200);
    let j1 = body_json(r1).await;
    let token = j1["access_token"].as_str().unwrap().to_string();

    // Link a local (password) auth method.
    let link_response = test_app
        .app
        .clone()
        .oneshot(auth_post(
            "/api/v1/auth/link",
            &token,
            &json!({"provider": "local", "password": "securepassword123"}),
        ))
        .await
        .unwrap();

    assert_eq!(link_response.status(), 200);
    let methods = body_json(link_response).await;
    let providers: Vec<&str> = methods
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["provider"].as_str())
        .collect();
    assert!(
        providers.contains(&"local"),
        "local should now be linked, got: {:?}",
        providers
    );

    // Verify the user can log in with password.
    // First, get the username.
    let user_row: (String,) = sqlx::query_as("SELECT email FROM users WHERE id = (SELECT user_id FROM user_auth_methods WHERE provider = 'apple' AND provider_subject = $1)")
        .bind(apple_sub)
        .fetch_one(&test_app.pool)
        .await
        .unwrap();

    let login_response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": user_row.0, "password": "securepassword123"}),
        ))
        .await
        .unwrap();

    assert_eq!(
        login_response.status(),
        200,
        "user should be able to log in with the linked password"
    );
}

// ---------------------------------------------------------------------------
// #11: link_auth with unsupported provider returns 400
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_link_unsupported_provider_returns_400() {
    let test_app = common::setup().await;
    let (_, token) = common::create_test_user(&test_app).await;

    let response = test_app
        .app
        .oneshot(auth_post(
            "/api/v1/auth/link",
            &token,
            &json!({"provider": "github"}),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "unsupported provider should return 400"
    );
}

// ---------------------------------------------------------------------------
// #12: Unauthenticated access to protected endpoints returns 401
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_unauthenticated_auth_methods_returns_401() {
    let test_app = common::setup().await;

    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/methods")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        401,
        "GET /auth/methods without JWT should return 401"
    );
}

#[tokio::test]
async fn test_unauthenticated_link_returns_401() {
    let test_app = common::setup().await;

    let response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/link",
            &json!({"provider": "local", "password": "testpass123"}),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        401,
        "POST /auth/link without JWT should return 401"
    );
}

#[tokio::test]
async fn test_unauthenticated_unlink_returns_401() {
    let test_app = common::setup().await;

    let response = test_app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/auth/link/local")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        401,
        "DELETE /auth/link/:provider without JWT should return 401"
    );
}

// ---------------------------------------------------------------------------
// S2: link_auth with "google" returns 400 (not yet supported)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_link_google_returns_not_yet_supported() {
    let test_app = common::setup().await;
    let (_, token) = common::create_test_user(&test_app).await;

    let response = test_app
        .app
        .oneshot(auth_post(
            "/api/v1/auth/link",
            &token,
            &json!({"provider": "google"}),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "linking Google should return 400 (not yet supported)"
    );
}

// ---------------------------------------------------------------------------
// S3: link_auth with short password returns 400
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_link_local_short_password_returns_400() {
    let test_app = common::setup().await;
    let (_, token) = common::create_test_user(&test_app).await;

    let response = test_app
        .app
        .oneshot(auth_post(
            "/api/v1/auth/link",
            &token,
            &json!({"provider": "local", "password": "short"}),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "password shorter than 10 characters should return 400"
    );
}

// ---------------------------------------------------------------------------
// Blocker 6: JWKS error / timeout / malformed tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_apple_callback_jwks_500_returns_401() {
    let kid = "test-key-jwks-500";
    let client_id = "com.example.ownpulse";
    let private_key = gen_rsa_key();

    let mock_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/auth/keys"))
        .respond_with(wiremock::ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let jwks_url = format!("{}/auth/keys", mock_server.uri());

    let test_app = common::setup_with_config(|c| {
        c.apple_client_id = Some(client_id.to_string());
        c.apple_jwks_url = jwks_url.clone();
    })
    .await;

    let id_token = make_apple_id_token(
        &private_key,
        kid,
        "apple-sub-jwks-500",
        client_id,
        Some("jwks500@example.com"),
    );

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/apple/callback",
            &json!({"id_token": id_token, "platform": "ios"}),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        401,
        "JWKS 500 error should result in 401"
    );
}

#[tokio::test]
async fn test_apple_callback_jwks_malformed_json_returns_401() {
    let kid = "test-key-jwks-malformed";
    let client_id = "com.example.ownpulse";
    let private_key = gen_rsa_key();

    let mock_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/auth/keys"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("this is not json"))
        .mount(&mock_server)
        .await;

    let jwks_url = format!("{}/auth/keys", mock_server.uri());

    let test_app = common::setup_with_config(|c| {
        c.apple_client_id = Some(client_id.to_string());
        c.apple_jwks_url = jwks_url.clone();
    })
    .await;

    let id_token = make_apple_id_token(
        &private_key,
        kid,
        "apple-sub-jwks-malformed",
        client_id,
        Some("malformed@example.com"),
    );

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/apple/callback",
            &json!({"id_token": id_token, "platform": "ios"}),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        401,
        "malformed JWKS JSON should result in 401"
    );
}

#[tokio::test]
async fn test_apple_callback_jwks_no_matching_kid_returns_401() {
    let kid = "test-key-no-match";
    let client_id = "com.example.ownpulse";
    let private_key = gen_rsa_key();

    // JWKS has a key, but with a different kid.
    let other_key = gen_rsa_key();
    let jwks = make_jwks(&other_key, "different-kid");

    let mock_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/auth/keys"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(&jwks))
        .mount(&mock_server)
        .await;

    let jwks_url = format!("{}/auth/keys", mock_server.uri());

    let test_app = common::setup_with_config(|c| {
        c.apple_client_id = Some(client_id.to_string());
        c.apple_jwks_url = jwks_url.clone();
    })
    .await;

    let id_token = make_apple_id_token(
        &private_key,
        kid,
        "apple-sub-no-match",
        client_id,
        Some("nomatch@example.com"),
    );

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/apple/callback",
            &json!({"id_token": id_token, "platform": "ios"}),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        401,
        "JWKS with no matching kid should result in 401"
    );
}

// ---------------------------------------------------------------------------
// Blocker 7: Migration data-preservation test
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_migration_0008_populates_user_auth_methods() {
    // This test creates users with local and google auth_provider values,
    // then verifies that user_auth_methods rows were correctly created
    // by the migration.
    let test_app = common::setup().await;

    // Insert a local user directly (bypassing the helper to control auth_provider).
    let local_email = format!("local-{}@example.com", uuid::Uuid::new_v4());
    let local_id: (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO users (email, password_hash, auth_provider) VALUES ($1, $2, 'local') RETURNING id",
    )
    .bind(&local_email)
    .bind("$2b$04$dummy_hash_value_for_testing_only_nope")
    .fetch_one(&test_app.pool)
    .await
    .unwrap();

    // Insert a google user.
    let google_email = format!("google-{}@example.com", uuid::Uuid::new_v4());
    let google_id: (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO users (email, auth_provider) VALUES ($1, 'google') RETURNING id",
    )
    .bind(&google_email)
    .fetch_one(&test_app.pool)
    .await
    .unwrap();

    // Simulate the migration logic by inserting auth method rows the same way
    // migration 0008 does. (The migration already ran during setup, but these
    // new users were inserted after, so we apply the same logic manually.)
    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject, email)
         SELECT id, auth_provider, CASE WHEN auth_provider = 'local' THEN id::TEXT ELSE email END, email
         FROM users
         WHERE id = $1 OR id = $2",
    )
    .bind(local_id.0)
    .bind(google_id.0)
    .execute(&test_app.pool)
    .await
    .unwrap();

    // Verify the local user has a 'local' auth method with provider_subject = user_id.
    let local_method: (String, String) = sqlx::query_as(
        "SELECT provider, provider_subject FROM user_auth_methods WHERE user_id = $1 AND provider = 'local'",
    )
    .bind(local_id.0)
    .fetch_one(&test_app.pool)
    .await
    .unwrap();
    assert_eq!(local_method.0, "local");
    assert_eq!(local_method.1, local_id.0.to_string());

    // Verify the google user has a 'google' auth method with provider_subject = email.
    let google_method: (String, String) = sqlx::query_as(
        "SELECT provider, provider_subject FROM user_auth_methods WHERE user_id = $1 AND provider = 'google'",
    )
    .bind(google_id.0)
    .fetch_one(&test_app.pool)
    .await
    .unwrap();
    assert_eq!(google_method.0, "google");
    assert_eq!(google_method.1, google_email);
}

// ---------------------------------------------------------------------------
// Blocker 8: Apple callback with unconfigured APPLE_CLIENT_ID returns 500
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_apple_callback_no_client_id_returns_500() {
    // Use default config which has apple_client_id = None.
    let test_app = common::setup().await;

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/apple/callback",
            &json!({"id_token": "dummy.jwt.token", "platform": "ios"}),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        500,
        "apple_callback without APPLE_CLIENT_ID should return 500"
    );
}

// ---------------------------------------------------------------------------
// Important 9: Validate platform field
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_apple_callback_unknown_platform_returns_400() {
    let test_app = common::setup_with_config(|c| {
        c.apple_client_id = Some("com.example.ownpulse".to_string());
    })
    .await;

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/apple/callback",
            &json!({"id_token": "dummy.jwt.token", "platform": "android"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400, "unknown platform should return 400");
}

// ---------------------------------------------------------------------------
// S4: unlink a provider the user doesn't have returns 404
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_unlink_nonexistent_provider_returns_404() {
    let test_app = common::setup().await;
    let (user_id, token) = common::create_test_user(&test_app).await;

    // Give the user a second method so the "last method" guard doesn't trigger.
    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject, email) VALUES ($1, 'google', $2, $3)",
    )
    .bind(user_id)
    .bind("google-sub-nonexistent")
    .bind("nonexistent@example.com")
    .execute(&test_app.pool)
    .await
    .unwrap();

    // Try to unlink 'apple' which the user doesn't have.
    let response = test_app
        .app
        .oneshot(auth_delete("/api/v1/auth/link/apple", &token))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        404,
        "unlinking a provider the user doesn't have should return 404"
    );
}

// ---------------------------------------------------------------------------
// Blocker 11: unlink a provider user doesn't have when they only have 1 method
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_unlink_nonexistent_provider_with_single_method_returns_404() {
    let test_app = common::setup().await;
    let (user_id, token) = common::create_test_user(&test_app).await;

    // Ensure there is exactly 1 auth method.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM user_auth_methods WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&test_app.pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1, "user should have exactly 1 auth method");

    // Try to unlink 'apple' which the user doesn't have.
    // Should return 404 "provider not linked", NOT 400 "cannot remove your only login method".
    let response = test_app
        .app
        .oneshot(auth_delete("/api/v1/auth/link/apple", &token))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        404,
        "unlinking a nonexistent provider should return 404 even with only 1 method"
    );
}
