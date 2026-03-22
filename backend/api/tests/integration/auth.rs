// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::body::Body;
use http::Request;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::common;

/// Helper: collect the response body into a parsed JSON value.
async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Helper: build a POST request with JSON body.
fn post_json(uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .unwrap()
}

/// Helper: build a POST request with a cookie header.
fn post_with_cookie(uri: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap()
}

/// Helper: insert a local user into the database with a bcrypt-hashed password.
async fn insert_test_user(pool: &sqlx::PgPool, username: &str, password: &str) -> uuid::Uuid {
    let hash = bcrypt::hash(password, 4).expect("bcrypt hash failed");
    let row: (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO users (username, password_hash, auth_provider) VALUES ($1, $2, 'local') RETURNING id",
    )
    .bind(username)
    .bind(&hash)
    .fetch_one(pool)
    .await
    .expect("failed to insert test user");
    row.0
}

/// Helper: extract the refresh_token value from Set-Cookie headers.
fn extract_refresh_cookie(response: &axum::response::Response) -> String {
    response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(|cookie| {
            cookie
                .split(';')
                .next()
                .and_then(|first| first.strip_prefix("refresh_token="))
                .map(|s| s.to_string())
        })
        .next()
        .expect("no refresh_token cookie found")
}

#[tokio::test]
async fn test_login_with_valid_credentials() {
    let test_app = common::setup().await;
    insert_test_user(&test_app.pool, "alice", "correctpassword").await;

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"username": "alice", "password": "correctpassword"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    // Verify Set-Cookie header contains refresh_token
    let cookie_value = extract_refresh_cookie(&response);
    assert!(!cookie_value.is_empty(), "refresh_token cookie should not be empty");

    let json = body_json(response).await;
    assert!(json["access_token"].is_string());
    assert!(!json["access_token"].as_str().unwrap().is_empty());
    assert_eq!(json["token_type"], "Bearer");
}

#[tokio::test]
async fn test_login_with_wrong_password() {
    let test_app = common::setup().await;
    insert_test_user(&test_app.pool, "bob", "realpassword").await;

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"username": "bob", "password": "wrongpassword"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_login_with_nonexistent_user() {
    let test_app = common::setup().await;

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"username": "nobody", "password": "whatever"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_refresh_token_rotation() {
    let test_app = common::setup().await;
    insert_test_user(&test_app.pool, "carol", "mypassword").await;

    // Login to get the refresh cookie
    let login_response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"username": "carol", "password": "mypassword"}),
        ))
        .await
        .unwrap();

    assert_eq!(login_response.status(), 200);
    let refresh_token = extract_refresh_cookie(&login_response);

    // Use the refresh cookie to get a new access token
    let refresh_response = test_app
        .app
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={refresh_token}"),
        ))
        .await
        .unwrap();

    assert_eq!(refresh_response.status(), 200);

    // Verify we got a new access token and a new refresh cookie
    let new_refresh = extract_refresh_cookie(&refresh_response);
    assert!(!new_refresh.is_empty());
    assert_ne!(refresh_token, new_refresh, "refresh token should be rotated");

    let json = body_json(refresh_response).await;
    assert!(json["access_token"].is_string());
    assert!(!json["access_token"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_refresh_with_no_cookie() {
    let test_app = common::setup().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_logout_clears_refresh_token() {
    let test_app = common::setup().await;
    insert_test_user(&test_app.pool, "dave", "secret123").await;

    // Login
    let login_response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"username": "dave", "password": "secret123"}),
        ))
        .await
        .unwrap();

    assert_eq!(login_response.status(), 200);
    let refresh_token = extract_refresh_cookie(&login_response);

    // Logout with that refresh cookie
    let logout_response = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/logout",
            &format!("refresh_token={refresh_token}"),
        ))
        .await
        .unwrap();

    assert_eq!(logout_response.status(), 204);

    // Try to refresh with the old token — should fail
    let refresh_response = test_app
        .app
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={refresh_token}"),
        ))
        .await
        .unwrap();

    assert_eq!(refresh_response.status(), 401);
}

#[tokio::test]
async fn test_refresh_with_json_body() {
    let test_app = common::setup().await;
    insert_test_user(&test_app.pool, "frank", "bodyrefresh").await;

    // Login to get a refresh token
    let login_response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"username": "frank", "password": "bodyrefresh"}),
        ))
        .await
        .unwrap();

    assert_eq!(login_response.status(), 200);
    let refresh_token = extract_refresh_cookie(&login_response);

    // Refresh using JSON body instead of cookie
    let refresh_response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/refresh",
            &json!({"refresh_token": refresh_token}),
        ))
        .await
        .unwrap();

    assert_eq!(refresh_response.status(), 200);

    let json = body_json(refresh_response).await;
    assert!(json["access_token"].is_string());
    assert!(!json["access_token"].as_str().unwrap().is_empty());
    assert_eq!(json["token_type"], "Bearer");
}

#[tokio::test]
async fn test_google_callback_ios_redirects_to_custom_scheme() {
    let test_app = common::setup().await;

    // Start WireMock for Google token exchange + userinfo
    let mock_server = wiremock::MockServer::start().await;

    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/token"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "mock-google-access-token",
            "id_token": "mock-id-token",
            "refresh_token": "mock-google-refresh-token"
        })))
        .mount(&mock_server)
        .await;

    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/userinfo"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
            "sub": "google-123",
            "email": "iosuser@example.com",
            "name": "iOS User"
        })))
        .mount(&mock_server)
        .await;

    // Build a test app with Google config pointing to WireMock
    let config = api::config::Config {
        database_url: "unused".to_string(),
        jwt_secret: "test-jwt-secret-at-least-32-bytes-long".to_string(),
        jwt_expiry_seconds: 3600,
        refresh_token_expiry_seconds: 2_592_000,
        google_client_id: Some("test-client-id".to_string()),
        google_client_secret: Some("test-client-secret".to_string()),
        google_redirect_uri: Some("http://localhost/callback".to_string()),
        google_token_url: format!("{}/token", mock_server.uri()),
        google_userinfo_url: format!("{}/userinfo", mock_server.uri()),
        apple_client_id: None,
        apple_jwks_url: api::config::default_apple_jwks_url(),
        garmin_client_id: None,
        garmin_client_secret: None,
        oura_client_id: None,
        oura_client_secret: None,
        dexcom_client_id: None,
        dexcom_client_secret: None,
        encryption_key: "0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
        encryption_key_previous: None,
        storage_path: None,
        app_user: None,
        app_password_hash: None,
        data_region: "us".to_string(),
        web_origin: "http://localhost:5173".to_string(),
        rust_log: "info".to_string(),
    };

    let state = api::AppState {
        pool: test_app.pool.clone(),
        config,
        http_client: reqwest::Client::new(),
    };

    let app = api::build_app_without_metrics(state);

    // Call the callback with state=ios (iOS bypasses CSRF)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/google/callback?code=test-auth-code&state=ios")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should be a redirect (303 See Other from axum::Redirect::to)
    assert!(
        response.status().is_redirection(),
        "expected redirect, got {}",
        response.status()
    );

    let location = response
        .headers()
        .get("location")
        .expect("missing location header")
        .to_str()
        .unwrap();

    assert!(
        location.starts_with("ownpulse://auth#"),
        "expected custom scheme redirect, got: {location}"
    );
    assert!(
        location.contains("token="),
        "redirect should contain token param"
    );
    assert!(
        location.contains("refresh_token="),
        "redirect should contain refresh_token param"
    );
}

#[tokio::test]
async fn test_google_callback_web_redirects_with_cookies() {
    let test_app = common::setup().await;

    let mock_server = wiremock::MockServer::start().await;

    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/token"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "mock-google-access-token",
            "id_token": "mock-id-token",
            "refresh_token": "mock-google-refresh-token"
        })))
        .mount(&mock_server)
        .await;

    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/userinfo"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
            "sub": "google-456",
            "email": "webuser@example.com",
            "name": "Web User"
        })))
        .mount(&mock_server)
        .await;

    let config = api::config::Config {
        database_url: "unused".to_string(),
        jwt_secret: "test-jwt-secret-at-least-32-bytes-long".to_string(),
        jwt_expiry_seconds: 3600,
        refresh_token_expiry_seconds: 2_592_000,
        google_client_id: Some("test-client-id".to_string()),
        google_client_secret: Some("test-client-secret".to_string()),
        google_redirect_uri: Some("http://localhost/callback".to_string()),
        google_token_url: format!("{}/token", mock_server.uri()),
        google_userinfo_url: format!("{}/userinfo", mock_server.uri()),
        apple_client_id: None,
        apple_jwks_url: api::config::default_apple_jwks_url(),
        garmin_client_id: None,
        garmin_client_secret: None,
        oura_client_id: None,
        oura_client_secret: None,
        dexcom_client_id: None,
        dexcom_client_secret: None,
        encryption_key: "0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
        encryption_key_previous: None,
        storage_path: None,
        app_user: None,
        app_password_hash: None,
        data_region: "us".to_string(),
        web_origin: "http://localhost:5173".to_string(),
        rust_log: "info".to_string(),
    };

    let state = api::AppState {
        pool: test_app.pool.clone(),
        config,
        http_client: reqwest::Client::new(),
    };

    let app = api::build_app_without_metrics(state);

    let csrf_state = "test-csrf-state-value";

    // Call with matching state and oauth_state cookie (CSRF validated)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/auth/google/callback?code=test-auth-code&state={}",
                    csrf_state
                ))
                .header("cookie", format!("oauth_state={}", csrf_state))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert!(response.status().is_redirection());

    let location = response
        .headers()
        .get("location")
        .expect("missing location header")
        .to_str()
        .unwrap();

    // Web redirect should NOT contain tokens in the URL
    assert!(
        location.starts_with("http://localhost:5173/?auth=success"),
        "expected web origin redirect without tokens, got: {location}"
    );
    assert!(
        !location.contains("token="),
        "redirect URL should NOT contain tokens"
    );

    // Web redirects SHOULD set cookies for both access_token and refresh_token
    let set_cookies: Vec<&str> = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();

    assert!(
        set_cookies.iter().any(|c| c.starts_with("access_token=")),
        "web redirect should set access_token cookie, got: {:?}",
        set_cookies
    );
    assert!(
        set_cookies.iter().any(|c| c.starts_with("refresh_token=")),
        "web redirect should set refresh_token cookie, got: {:?}",
        set_cookies
    );
}

#[tokio::test]
async fn test_google_callback_rejects_mismatched_csrf_state() {
    let test_app = common::setup().await;

    let config = api::config::Config {
        database_url: "unused".to_string(),
        jwt_secret: "test-jwt-secret-at-least-32-bytes-long".to_string(),
        jwt_expiry_seconds: 3600,
        refresh_token_expiry_seconds: 2_592_000,
        google_client_id: Some("test-client-id".to_string()),
        google_client_secret: Some("test-client-secret".to_string()),
        google_redirect_uri: Some("http://localhost/callback".to_string()),
        google_token_url: "https://oauth2.googleapis.com/token".to_string(),
        google_userinfo_url: "https://www.googleapis.com/oauth2/v3/userinfo".to_string(),
        apple_client_id: None,
        apple_jwks_url: api::config::default_apple_jwks_url(),
        garmin_client_id: None,
        garmin_client_secret: None,
        oura_client_id: None,
        oura_client_secret: None,
        dexcom_client_id: None,
        dexcom_client_secret: None,
        encryption_key: "0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
        encryption_key_previous: None,
        storage_path: None,
        app_user: None,
        app_password_hash: None,
        data_region: "us".to_string(),
        web_origin: "http://localhost:5173".to_string(),
        rust_log: "info".to_string(),
    };

    let state = api::AppState {
        pool: test_app.pool.clone(),
        config,
        http_client: reqwest::Client::new(),
    };

    let app = api::build_app_without_metrics(state);

    // Send with mismatched state values — should be rejected before token exchange
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/google/callback?code=test-auth-code&state=attacker-state")
                .header("cookie", "oauth_state=real-state")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "mismatched CSRF state should return 400"
    );
}

#[tokio::test]
async fn test_login_returns_decodable_jwt() {
    let test_app = common::setup().await;
    let user_id = insert_test_user(&test_app.pool, "eve", "jwttest").await;

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"username": "eve", "password": "jwttest"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let json = body_json(response).await;
    let access_token = json["access_token"].as_str().unwrap();

    // Decode the JWT using the same secret as the test config
    let claims = api::auth::jwt::decode_access_token(
        access_token,
        "test-jwt-secret-at-least-32-bytes-long",
    )
    .expect("JWT should decode successfully");

    assert_eq!(claims.sub, user_id);
    assert!(claims.exp > chrono::Utc::now().timestamp());
}

/// Regression test: presenting an already-rotated refresh token should return 401.
/// This verifies replay detection — once a token is rotated, the old one is invalid.
#[tokio::test]
async fn test_rotated_refresh_token_returns_401() {
    let test_app = common::setup().await;
    insert_test_user(&test_app.pool, "grace", "replaytest").await;

    // Step 1: Login to get the initial refresh token
    let login_response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"username": "grace", "password": "replaytest"}),
        ))
        .await
        .unwrap();

    assert_eq!(login_response.status(), 200);
    let old_refresh_token = extract_refresh_cookie(&login_response);

    // Step 2: Use the refresh token to rotate it (get a new one)
    let refresh_response = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={old_refresh_token}"),
        ))
        .await
        .unwrap();

    assert_eq!(refresh_response.status(), 200);
    let new_refresh_token = extract_refresh_cookie(&refresh_response);
    assert_ne!(old_refresh_token, new_refresh_token, "token should have rotated");

    // Step 3: Present the OLD (already-rotated) refresh token — should be rejected
    let replay_response = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={old_refresh_token}"),
        ))
        .await
        .unwrap();

    assert_eq!(
        replay_response.status(),
        401,
        "presenting an already-rotated refresh token should return 401"
    );

    // Step 4: Verify the new token still works
    let valid_response = test_app
        .app
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={new_refresh_token}"),
        ))
        .await
        .unwrap();

    assert_eq!(
        valid_response.status(),
        200,
        "the current valid refresh token should still work"
    );
}
