// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::body::Body;
use http::Request;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::common;

/// Helper: collect the response body into a parsed JSON value.
async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
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
async fn insert_test_user(pool: &sqlx::PgPool, email: &str, password: &str) -> uuid::Uuid {
    let hash = bcrypt::hash(password, 4).expect("bcrypt hash failed");
    let row: (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO users (email, password_hash, auth_provider) VALUES ($1, $2, 'local') RETURNING id",
    )
    .bind(email)
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
    insert_test_user(&test_app.pool, "alice@example.com", "correctpassword").await;

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": "alice@example.com", "password": "correctpassword"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    // Verify Set-Cookie header contains refresh_token
    let cookie_value = extract_refresh_cookie(&response);
    assert!(
        !cookie_value.is_empty(),
        "refresh_token cookie should not be empty"
    );

    let json = body_json(response).await;
    assert!(json["access_token"].is_string());
    assert!(!json["access_token"].as_str().unwrap().is_empty());
    assert_eq!(json["token_type"], "Bearer");
}

#[tokio::test]
async fn test_login_with_wrong_password() {
    let test_app = common::setup().await;
    insert_test_user(&test_app.pool, "bob@example.com", "realpassword").await;

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": "bob@example.com", "password": "wrongpassword"}),
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
            &json!({"email": "nobody@example.com", "password": "whatever"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_refresh_token_rotation() {
    let test_app = common::setup().await;
    insert_test_user(&test_app.pool, "carol@example.com", "mypassword").await;

    // Login to get the refresh cookie
    let login_response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": "carol@example.com", "password": "mypassword"}),
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
    assert_ne!(
        refresh_token, new_refresh,
        "refresh token should be rotated"
    );

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
    insert_test_user(&test_app.pool, "dave@example.com", "secret123").await;

    // Login
    let login_response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": "dave@example.com", "password": "secret123"}),
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
    insert_test_user(&test_app.pool, "frank@example.com", "bodyrefresh").await;

    // Login to get a refresh token
    let login_response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": "frank@example.com", "password": "bodyrefresh"}),
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

/// Build a Config pointing Google endpoints at the given WireMock server URI,
/// sharing the pool from the outer TestApp.
fn google_config(mock_uri: &str) -> api::config::Config {
    api::config::Config {
        database_url: "unused".to_string(),
        jwt_secret: "test-jwt-secret-at-least-32-bytes-long".to_string(),
        jwt_expiry_seconds: 3600,
        refresh_token_expiry_seconds: 2_592_000,
        google_client_id: Some("test-client-id".to_string()),
        google_client_secret: Some("test-client-secret".to_string()),
        google_redirect_uri: Some("http://localhost/callback".to_string()),
        google_token_url: format!("{mock_uri}/token"),
        google_userinfo_url: format!("{mock_uri}/userinfo"),
        apple_client_id: None,
        apple_jwks_url: api::config::default_apple_jwks_url(),
        garmin_client_id: None,
        garmin_client_secret: None,
        garmin_base_url: None,
        oura_client_id: None,
        oura_client_secret: None,
        oura_api_base_url: None,
        oura_auth_base_url: None,
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
        require_invite: false,
        ios_min_version: None,
        ios_force_upgrade_below: None,
        smtp_host: None,
        smtp_port: 2587,
        smtp_username: None,
        smtp_password: None,
        smtp_from: None,
    }
}

#[tokio::test]
async fn test_google_callback_pkce_redirects_to_custom_scheme() {
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

    let (event_tx, _) = tokio::sync::broadcast::channel(256);
    let state = api::AppState {
        pool: test_app.pool.clone(),
        config: google_config(&mock_server.uri()),
        http_client: reqwest::Client::new(),
        migrations_ready: common::migrations_ready_flag(),
        event_tx,
    };
    let app = api::build_app_without_metrics(state);

    // Native app PKCE flow: sends code_verifier, no CSRF cookie needed.
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/google/callback?code=test-auth-code&code_verifier=dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk")
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

/// Shared helper: start a WireMock server with Google token + userinfo stubs.
async fn setup_google_mock(sub: &str, email: &str) -> wiremock::MockServer {
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
            "sub": sub,
            "email": email,
            "name": "Test User"
        })))
        .mount(&mock_server)
        .await;

    mock_server
}

/// Regression: the old `state=ios` bypass must no longer skip CSRF validation.
/// A request with `state=ios` but no `code_verifier` is treated as a web flow
/// and rejected because no `oauth_state` cookie is present.
#[tokio::test]
async fn test_google_callback_state_ios_no_longer_bypasses_csrf() {
    let test_app = common::setup().await;

    let config = api::config::Config {
        database_url: "unused".to_string(),
        jwt_secret: "test-jwt-secret-at-least-32-bytes-long".to_string(),
        jwt_expiry_seconds: 3600,
        refresh_token_expiry_seconds: 2_592_000,
        google_client_id: Some("test-client-id".to_string()),
        google_client_secret: Some("test-client-secret".to_string()),
        google_redirect_uri: Some("http://localhost/callback".to_string()),
        // Point at a URL that should never be reached — CSRF check fires first.
        google_token_url: "http://127.0.0.1:0/token".to_string(),
        google_userinfo_url: "http://127.0.0.1:0/userinfo".to_string(),
        apple_client_id: None,
        apple_jwks_url: api::config::default_apple_jwks_url(),
        garmin_client_id: None,
        garmin_client_secret: None,
        garmin_base_url: None,
        oura_client_id: None,
        oura_client_secret: None,
        oura_api_base_url: None,
        oura_auth_base_url: None,
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
        require_invite: false,
        ios_min_version: None,
        ios_force_upgrade_below: None,
        smtp_host: None,
        smtp_port: 2587,
        smtp_username: None,
        smtp_password: None,
        smtp_from: None,
    };

    let (event_tx, _) = tokio::sync::broadcast::channel(256);
    let state = api::AppState {
        pool: test_app.pool.clone(),
        config,
        http_client: reqwest::Client::new(),
        migrations_ready: common::migrations_ready_flag(),
        event_tx,
    };
    let app = api::build_app_without_metrics(state);

    // state=ios without code_verifier — the bypass has been removed.
    // The handler treats this as a web flow and rejects it: no oauth_state cookie.
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

    assert_eq!(
        response.status(),
        400,
        "state=ios without code_verifier should no longer bypass CSRF — expected 400, got {}",
        response.status()
    );
}

/// A callback with neither `code_verifier` nor a valid `oauth_state` cookie
/// must be rejected before any token exchange occurs.
#[tokio::test]
async fn test_google_callback_no_verifier_no_cookie_returns_400() {
    let test_app = common::setup().await;

    let config = api::config::Config {
        database_url: "unused".to_string(),
        jwt_secret: "test-jwt-secret-at-least-32-bytes-long".to_string(),
        jwt_expiry_seconds: 3600,
        refresh_token_expiry_seconds: 2_592_000,
        google_client_id: Some("test-client-id".to_string()),
        google_client_secret: Some("test-client-secret".to_string()),
        google_redirect_uri: Some("http://localhost/callback".to_string()),
        // Point at a URL that should never be reached — CSRF check fires first.
        google_token_url: "http://127.0.0.1:0/token".to_string(),
        google_userinfo_url: "http://127.0.0.1:0/userinfo".to_string(),
        apple_client_id: None,
        apple_jwks_url: api::config::default_apple_jwks_url(),
        garmin_client_id: None,
        garmin_client_secret: None,
        garmin_base_url: None,
        oura_client_id: None,
        oura_client_secret: None,
        oura_api_base_url: None,
        oura_auth_base_url: None,
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
        require_invite: false,
        ios_min_version: None,
        ios_force_upgrade_below: None,
        smtp_host: None,
        smtp_port: 2587,
        smtp_username: None,
        smtp_password: None,
        smtp_from: None,
    };

    let (event_tx, _) = tokio::sync::broadcast::channel(256);
    let state = api::AppState {
        pool: test_app.pool.clone(),
        config,
        http_client: reqwest::Client::new(),
        migrations_ready: common::migrations_ready_flag(),
        event_tx,
    };
    let app = api::build_app_without_metrics(state);

    // No code_verifier, no oauth_state cookie — must be rejected.
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/google/callback?code=test-auth-code&state=some-state")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "callback without code_verifier and without oauth_state cookie should return 400"
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

    let (event_tx, _) = tokio::sync::broadcast::channel(256);
    let state = api::AppState {
        pool: test_app.pool.clone(),
        config: google_config(&mock_server.uri()),
        http_client: reqwest::Client::new(),
        migrations_ready: common::migrations_ready_flag(),
        event_tx,
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

    // Point token URL at a port-0 address — the handler must reject before
    // it ever makes a network call.
    let (event_tx, _) = tokio::sync::broadcast::channel(256);
    let state = api::AppState {
        pool: test_app.pool.clone(),
        config: google_config("http://127.0.0.1:0"),
        http_client: reqwest::Client::new(),
        migrations_ready: common::migrations_ready_flag(),
        event_tx,
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
    let user_id = insert_test_user(&test_app.pool, "eve@example.com", "jwttest").await;

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": "eve@example.com", "password": "jwttest"}),
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
        "http://localhost:5173",
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
    insert_test_user(&test_app.pool, "grace@example.com", "replaytest").await;

    // Step 1: Login to get the initial refresh token
    let login_response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": "grace@example.com", "password": "replaytest"}),
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
    assert_ne!(
        old_refresh_token, new_refresh_token,
        "token should have rotated"
    );

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

// ---------------------------------------------------------------------------
// Google callback: email collision tests
// ---------------------------------------------------------------------------

/// When a local user already exists with the same email, a Google OAuth
/// registration (web flow) must redirect to /login?error=email_exists.
#[tokio::test]
async fn test_google_callback_email_collision_redirects_with_error() {
    let test_app = common::setup().await;
    let email = "collision@example.com";
    insert_test_user(&test_app.pool, email, "existingpass").await;

    let mock_server = setup_google_mock("google-collision-sub", email).await;

    let (event_tx, _) = tokio::sync::broadcast::channel(256);
    let state = api::AppState {
        pool: test_app.pool.clone(),
        config: google_config(&mock_server.uri()),
        http_client: reqwest::Client::new(),
        migrations_ready: common::migrations_ready_flag(),
        event_tx,
    };
    let app = api::build_app_without_metrics(state);

    let csrf_state = "csrf-collision-test";
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/auth/google/callback?code=test-auth-code&state={csrf_state}"
                ))
                .header("cookie", format!("oauth_state={csrf_state}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

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
        location.contains("/login?error=email_exists"),
        "expected email_exists error redirect, got: {location}"
    );
}

/// Email collision in PKCE flow redirects to ownpulse://auth?error=email_exists.
#[tokio::test]
async fn test_google_callback_email_collision_pkce_redirects_with_error() {
    let test_app = common::setup().await;
    let email = "pkce-collision@example.com";
    insert_test_user(&test_app.pool, email, "existingpass").await;

    let mock_server = setup_google_mock("google-pkce-collision-sub", email).await;

    let (event_tx, _) = tokio::sync::broadcast::channel(256);
    let state = api::AppState {
        pool: test_app.pool.clone(),
        config: google_config(&mock_server.uri()),
        http_client: reqwest::Client::new(),
        migrations_ready: common::migrations_ready_flag(),
        event_tx,
    };
    let app = api::build_app_without_metrics(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/google/callback?code=test-auth-code&code_verifier=dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

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
        location.starts_with("ownpulse://auth?error=email_exists"),
        "expected PKCE email_exists redirect, got: {location}"
    );
}

// ---------------------------------------------------------------------------
// Google link mode tests
// ---------------------------------------------------------------------------

/// Helper: create a user and return (user_id, access_token_cookie_value).
async fn create_user_with_access_token(pool: &sqlx::PgPool, email: &str) -> (uuid::Uuid, String) {
    let user_id = insert_test_user(pool, email, "linktest123").await;
    let token = api::auth::jwt::encode_access_token(
        user_id,
        "user",
        "test-jwt-secret-at-least-32-bytes-long",
        "http://localhost:5173",
        3600,
    )
    .expect("failed to encode JWT");
    (user_id, token)
}

/// An authenticated user can link their Google account.
#[tokio::test]
async fn test_google_link_flow_succeeds() {
    let test_app = common::setup().await;
    let (user_id, access_token) =
        create_user_with_access_token(&test_app.pool, "linker@example.com").await;

    let mock_server = setup_google_mock("google-link-sub", "linker-google@example.com").await;

    let (event_tx, _) = tokio::sync::broadcast::channel(256);
    let state = api::AppState {
        pool: test_app.pool.clone(),
        config: google_config(&mock_server.uri()),
        http_client: reqwest::Client::new(),
        migrations_ready: common::migrations_ready_flag(),
        event_tx,
    };
    let app = api::build_app_without_metrics(state);

    let csrf_nonce = "link-csrf-nonce";
    let csrf_state = format!("{csrf_nonce}:link");

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/auth/google/callback?code=test-auth-code&state={csrf_state}"
                ))
                .header(
                    "cookie",
                    format!("oauth_state={csrf_state}; access_token={access_token}"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

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
        location.contains("/settings?linked=google"),
        "expected settings?linked=google redirect, got: {location}"
    );

    // Verify the auth method was actually inserted in the database.
    let methods: Vec<(String,)> = sqlx::query_as(
        "SELECT provider FROM user_auth_methods WHERE user_id = $1 ORDER BY provider",
    )
    .bind(user_id)
    .fetch_all(&test_app.pool)
    .await
    .expect("failed to query auth methods");

    let providers: Vec<&str> = methods.iter().map(|r| r.0.as_str()).collect();
    assert!(
        providers.contains(&"google"),
        "expected google auth method, got: {providers:?}"
    );
}

/// When Google sub is already linked to a different user, redirect to
/// /settings?error=already_linked.
#[tokio::test]
async fn test_google_link_already_linked_to_other_user_fails() {
    let test_app = common::setup().await;

    // Create the first user and link google to them.
    let first_user_id = insert_test_user(&test_app.pool, "first@example.com", "password1").await;
    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject, email)
         VALUES ($1, 'google', $2, $3)",
    )
    .bind(first_user_id)
    .bind("google-already-linked-sub")
    .bind("first-google@example.com")
    .execute(&test_app.pool)
    .await
    .expect("failed to insert auth method");

    // Create a second user who wants to link the same google account.
    let (_second_user_id, access_token) =
        create_user_with_access_token(&test_app.pool, "second@example.com").await;

    let mock_server =
        setup_google_mock("google-already-linked-sub", "first-google@example.com").await;

    let (event_tx, _) = tokio::sync::broadcast::channel(256);
    let state = api::AppState {
        pool: test_app.pool.clone(),
        config: google_config(&mock_server.uri()),
        http_client: reqwest::Client::new(),
        migrations_ready: common::migrations_ready_flag(),
        event_tx,
    };
    let app = api::build_app_without_metrics(state);

    let csrf_nonce = "link-already-nonce";
    let csrf_state = format!("{csrf_nonce}:link");

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/auth/google/callback?code=test-auth-code&state={csrf_state}"
                ))
                .header(
                    "cookie",
                    format!("oauth_state={csrf_state}; access_token={access_token}"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

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
        location.contains("/settings?error=already_linked"),
        "expected already_linked error redirect, got: {location}"
    );
}

/// Without an access_token cookie, link mode redirects to /login?error=auth_required.
#[tokio::test]
async fn test_google_link_unauthenticated_redirects_to_login() {
    let test_app = common::setup().await;

    let mock_server = setup_google_mock("google-unauth-sub", "unauth@example.com").await;

    let (event_tx, _) = tokio::sync::broadcast::channel(256);
    let state = api::AppState {
        pool: test_app.pool.clone(),
        config: google_config(&mock_server.uri()),
        http_client: reqwest::Client::new(),
        migrations_ready: common::migrations_ready_flag(),
        event_tx,
    };
    let app = api::build_app_without_metrics(state);

    let csrf_nonce = "unauth-link-nonce";
    let csrf_state = format!("{csrf_nonce}:link");

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/auth/google/callback?code=test-auth-code&state={csrf_state}"
                ))
                .header("cookie", format!("oauth_state={csrf_state}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

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
        location.contains("/login?error=auth_required"),
        "expected auth_required error redirect, got: {location}"
    );
}

/// Linking the same Google account twice to the same user is idempotent —
/// no duplicate rows, and still redirects to /settings?linked=google.
#[tokio::test]
async fn test_google_link_idempotent_same_user() {
    let test_app = common::setup().await;
    let (user_id, access_token) =
        create_user_with_access_token(&test_app.pool, "idempotent@example.com").await;

    // Pre-link the Google account.
    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject, email)
         VALUES ($1, 'google', $2, $3)",
    )
    .bind(user_id)
    .bind("google-idempotent-sub")
    .bind("idempotent-google@example.com")
    .execute(&test_app.pool)
    .await
    .expect("failed to insert auth method");

    let mock_server =
        setup_google_mock("google-idempotent-sub", "idempotent-google@example.com").await;

    let (event_tx, _) = tokio::sync::broadcast::channel(256);
    let state = api::AppState {
        pool: test_app.pool.clone(),
        config: google_config(&mock_server.uri()),
        http_client: reqwest::Client::new(),
        migrations_ready: common::migrations_ready_flag(),
        event_tx,
    };
    let app = api::build_app_without_metrics(state);

    let csrf_nonce = "idempotent-nonce";
    let csrf_state = format!("{csrf_nonce}:link");

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/auth/google/callback?code=test-auth-code&state={csrf_state}"
                ))
                .header(
                    "cookie",
                    format!("oauth_state={csrf_state}; access_token={access_token}"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

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
        location.contains("/settings?linked=google"),
        "expected idempotent success redirect, got: {location}"
    );

    // Verify no duplicate rows.
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM user_auth_methods WHERE user_id = $1 AND provider = 'google'",
    )
    .bind(user_id)
    .fetch_one(&test_app.pool)
    .await
    .expect("failed to count auth methods");

    assert_eq!(count.0, 1, "expected exactly one google auth method row");
}

/// A disabled user attempting to link Google gets a 403.
#[tokio::test]
async fn test_google_link_disabled_user_fails() {
    let test_app = common::setup().await;
    let (user_id, access_token) =
        create_user_with_access_token(&test_app.pool, "disabled-linker@example.com").await;

    // Disable the user.
    sqlx::query("UPDATE users SET status = 'disabled' WHERE id = $1")
        .bind(user_id)
        .execute(&test_app.pool)
        .await
        .expect("failed to disable user");

    let mock_server = setup_google_mock(
        "google-disabled-link-sub",
        "disabled-linker-google@example.com",
    )
    .await;

    let (event_tx, _) = tokio::sync::broadcast::channel(256);
    let state = api::AppState {
        pool: test_app.pool.clone(),
        config: google_config(&mock_server.uri()),
        http_client: reqwest::Client::new(),
        migrations_ready: common::migrations_ready_flag(),
        event_tx,
    };
    let app = api::build_app_without_metrics(state);

    let csrf_nonce = "disabled-link-nonce";
    let csrf_state = format!("{csrf_nonce}:link");

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/auth/google/callback?code=test-auth-code&state={csrf_state}"
                ))
                .header(
                    "cookie",
                    format!("oauth_state={csrf_state}; access_token={access_token}"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        403,
        "disabled user should get 403, got {}",
        response.status()
    );
}

// ─── First-user bootstrap (invite bypass) ──────────────────────────────────────

#[tokio::test]
async fn test_register_first_user_without_invite_when_require_invite_enabled() {
    let test_app = common::setup_with_config(|cfg| {
        cfg.require_invite = true;
    })
    .await;

    // No users exist — registration should succeed without an invite code.
    let response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/register",
            &json!({
                "email": "first@example.com",
                "password": "strongpassword123"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        200,
        "first user should register without invite code"
    );
    let json = body_json(response).await;
    assert!(json["access_token"].is_string());
}

#[tokio::test]
async fn test_register_first_user_gets_admin_role() {
    let test_app = common::setup_with_config(|cfg| {
        cfg.require_invite = true;
    })
    .await;

    // No users exist — first user should be promoted to admin.
    let response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/register",
            &json!({
                "email": "admin-first@example.com",
                "password": "strongpassword123"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        200,
        "first user should register successfully"
    );
    let json = body_json(response).await;
    let access_token = json["access_token"].as_str().expect("access_token missing");

    // Decode the JWT to verify the role claim is "admin"
    let claims = api::auth::jwt::decode_access_token(
        access_token,
        "test-jwt-secret-at-least-32-bytes-long",
        "http://localhost:5173",
    )
    .expect("failed to decode access token");
    assert_eq!(
        claims.role, "admin",
        "first user should have admin role in JWT"
    );

    // Also verify the database row was updated
    let row: (String,) =
        sqlx::query_as("SELECT role FROM users WHERE email = 'admin-first@example.com'")
            .fetch_one(&test_app.pool)
            .await
            .expect("failed to query user");
    assert_eq!(
        row.0, "admin",
        "first user should have admin role in database"
    );
}

#[tokio::test]
async fn test_register_second_user_gets_user_role() {
    let test_app = common::setup_with_config(|cfg| {
        cfg.require_invite = false;
    })
    .await;

    // Create the first user (will become admin).
    let first_response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/register",
            &json!({
                "email": "first-user@example.com",
                "password": "strongpassword123"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(first_response.status(), 200);

    // Second user registration — should get "user" role, not "admin".
    let response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/register",
            &json!({
                "email": "second-user@example.com",
                "password": "strongpassword456"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        200,
        "second user should register successfully"
    );
    let json = body_json(response).await;
    let access_token = json["access_token"].as_str().expect("access_token missing");

    // Decode the JWT to verify the role claim is "user"
    let claims = api::auth::jwt::decode_access_token(
        access_token,
        "test-jwt-secret-at-least-32-bytes-long",
        "http://localhost:5173",
    )
    .expect("failed to decode access token");
    assert_eq!(
        claims.role, "user",
        "second user should have user role in JWT"
    );

    // Also verify the database row
    let row: (String,) =
        sqlx::query_as("SELECT role FROM users WHERE email = 'second-user@example.com'")
            .fetch_one(&test_app.pool)
            .await
            .expect("failed to query user");
    assert_eq!(
        row.0, "user",
        "second user should have user role in database"
    );
}

#[tokio::test]
async fn test_register_second_user_without_invite_fails_when_require_invite_enabled() {
    let test_app = common::setup_with_config(|cfg| {
        cfg.require_invite = true;
    })
    .await;

    // Insert an existing user so the table is no longer empty.
    insert_test_user(&test_app.pool, "existing@example.com", "somepassword").await;

    // Second registration without invite code should fail.
    let response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/register",
            &json!({
                "email": "second@example.com",
                "password": "strongpassword123"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "second user without invite should be rejected"
    );
    let json = body_json(response).await;
    assert!(
        json["error"]
            .as_str()
            .unwrap_or("")
            .contains("invite code required"),
        "error message should mention invite code requirement"
    );
}
