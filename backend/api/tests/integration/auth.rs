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
    // Body-based refresh (iOS) must return the rotated refresh token in the
    // body so the client can persist it. Without this, the client keeps
    // presenting the rotated token until the grace window closes and the
    // family is revoked.
    assert!(json["refresh_token"].is_string());
    let new_refresh = json["refresh_token"].as_str().unwrap();
    assert!(!new_refresh.is_empty());
    assert_ne!(
        new_refresh, refresh_token,
        "refresh token should be rotated, not echoed back"
    );
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
        google_calendar_redirect_uri: None,
        google_calendar_api_base_url: None,
        mychart_client_id: None,
        mychart_allow_insecure_urls: true,
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

/// Build the router the Google auth tests drive, on the caller's pool.
fn google_app(pool: &sqlx::PgPool, config: api::config::Config) -> axum::Router {
    let (event_tx, _) = tokio::sync::broadcast::channel(256);
    api::build_app_without_metrics(api::AppState {
        pool: pool.clone(),
        config,
        http_client: reqwest::Client::new(),
        migrations_ready: common::migrations_ready_flag(),
        event_tx,
    })
}

/// Read the `Location` header as a string.
fn location_of(response: &axum::response::Response) -> String {
    response
        .headers()
        .get("location")
        .expect("missing location header")
        .to_str()
        .unwrap()
        .to_string()
}

/// Collect the `Set-Cookie` values of a response.
fn set_cookies(response: &axum::response::Response) -> Vec<String> {
    response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .collect()
}

/// What a browser holds after `GET /auth/google/login`: the `state` Google
/// will echo back, and the name and value of the state cookie the browser
/// stores. The name is captured rather than assumed — it is `oauth_state` on
/// a plain-HTTP origin and `__Host-oauth_state` on HTTPS, and a test that
/// hardcoded one would pass while writer and reader diverged.
struct StartedLogin {
    state: String,
    state_cookie_name: String,
    state_cookie: String,
}

impl StartedLogin {
    /// The `Cookie` header a browser that started this flow would send.
    fn cookie_header(&self) -> String {
        format!("{}={}", self.state_cookie_name, self.state_cookie)
    }
}

/// Run the real initiation — `GET /auth/google/login{query}` — and pull the
/// state out of the Google authorization URL and the state cookie out of
/// `Set-Cookie`. Callback tests pair these instead of hand-crafting cookies,
/// so they exercise what actually ships.
async fn start_google_login(
    app: &axum::Router,
    query: &str,
    cookie: Option<&str>,
) -> (StartedLogin, axum::response::Response) {
    let mut builder = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/auth/google/login{query}"));
    if let Some(cookie) = cookie {
        builder = builder.header("cookie", cookie);
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert!(
        response.status().is_redirection(),
        "google/login should redirect, got {}",
        response.status()
    );

    let location = location_of(&response);
    let state = location
        .split("&state=")
        .nth(1)
        .and_then(|rest| rest.split('&').next())
        .expect("no state parameter in Google authorization URL")
        .to_string();

    let (state_cookie_name, state_cookie) = set_cookies(&response)
        .iter()
        .find_map(|cookie| {
            let (name, value) = cookie.split(';').next()?.split_once('=')?;
            name.ends_with("oauth_state")
                .then(|| (name.to_string(), value.to_string()))
        })
        .expect("no oauth_state cookie set by google/login");

    (
        StartedLogin {
            state,
            state_cookie_name,
            state_cookie,
        },
        response,
    )
}

/// Build the callback request a browser would make after Google redirects
/// back, optionally with extra cookies an attacker-controlled sibling
/// subdomain might have injected.
fn google_callback_request(state: &str, cookies: &str) -> Request<Body> {
    let mut builder = Request::builder().method("GET").uri(format!(
        "/api/v1/auth/google/callback?code=test-auth-code&state={state}"
    ));
    if !cookies.is_empty() {
        builder = builder.header("cookie", cookies);
    }
    builder.body(Body::empty()).unwrap()
}

/// Initiation stores the flow server-side and echoes the state in a
/// host-only-shaped cookie (bare name here because the test origin is HTTP).
#[tokio::test]
async fn test_google_login_sets_state_cookie_and_row() {
    let test_app = common::setup().await;
    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));

    let (started, response) = start_google_login(&app, "", None).await;

    assert_eq!(
        started.state, started.state_cookie,
        "the state cookie must carry the same value as the state parameter"
    );
    let cookie = set_cookies(&response)
        .into_iter()
        .find(|c| c.starts_with("oauth_state="))
        .unwrap();
    assert!(cookie.contains("HttpOnly"), "state cookie must be HttpOnly");
    assert!(
        cookie.contains("Path=/;"),
        "state cookie must be Path=/ so the __Host- prefix applies on HTTPS, got: {cookie}"
    );
    assert!(
        !set_cookies(&response)
            .iter()
            .any(|c| c.starts_with("oauth_platform=") || c.starts_with("invite_code=")),
        "platform and invite code must live in the row, not in cookies"
    );

    let state_uuid = uuid::Uuid::parse_str(&started.state).expect("state should be a UUID");
    let row: (bool, Option<String>, Option<uuid::Uuid>) = sqlx::query_as(
        "SELECT is_native, invite_code, link_user_id FROM login_oauth_states WHERE state = $1",
    )
    .bind(state_uuid)
    .fetch_one(&test_app.pool)
    .await
    .expect("initiation should have stored a login_oauth_states row");
    assert_eq!(row, (false, None, None));
}

#[tokio::test]
async fn test_google_login_records_ios_platform_in_row() {
    let test_app = common::setup().await;
    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));

    let (started, _) = start_google_login(&app, "?platform=ios", None).await;

    let is_native: bool =
        sqlx::query_scalar("SELECT is_native FROM login_oauth_states WHERE state = $1")
            .bind(uuid::Uuid::parse_str(&started.state).unwrap())
            .fetch_one(&test_app.pool)
            .await
            .unwrap();
    assert!(is_native, "?platform=ios should be recorded on the row");
}

#[tokio::test]
async fn test_google_login_rejects_unknown_platform() {
    let test_app = common::setup().await;
    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/google/login?platform=android")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 400, "unknown platform should be a 400");
}

/// A near-miss `mode` must not silently mean "log in": the user asked to
/// link, and logging them in as whoever the Google identity maps to is a
/// different and worse outcome. (A percent-encoded `%6Cink` is not a near
/// miss — it decodes to `link` before the handler sees it, and is link mode.)
#[rstest::rstest]
#[case("Link")]
#[case("link%20")]
#[case("linkk")]
#[case("")]
#[tokio::test]
async fn test_google_login_rejects_near_miss_mode(#[case] mode: &str) {
    let test_app = common::setup().await;
    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/auth/google/login?mode={mode}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "mode={mode:?} must not fall through to login/register"
    );

    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM login_oauth_states")
        .fetch_one(&test_app.pool)
        .await
        .unwrap();
    assert_eq!(rows, 0, "a rejected initiation must not store a state");
}

/// The security property of the whole flow only holds if the name the login
/// handler writes is the name the callback reads, and if that name carries
/// `Secure` — browsers drop a `__Host-`-prefixed cookie without it. Drive a
/// full round trip on an HTTPS origin, where the prefixed name is in play.
#[tokio::test]
async fn test_google_flow_uses_host_prefixed_cookie_on_https_origin() {
    let test_app = common::setup().await;
    let mock_server = setup_google_mock("google-https", "https-user@example.com").await;
    let mut config = google_config(&mock_server.uri());
    config.web_origin = "https://app.ownpulse.health".to_string();
    let app = google_app(&test_app.pool, config);

    let (started, login_response) = start_google_login(&app, "", None).await;

    assert_eq!(
        started.state_cookie_name, "__Host-oauth_state",
        "an HTTPS origin must use the host-only cookie name"
    );
    let cookie = set_cookies(&login_response)
        .into_iter()
        .find(|c| c.starts_with("__Host-oauth_state="))
        .expect("no __Host-oauth_state cookie");
    // All three are required for the browser to accept the prefixed name.
    assert!(cookie.contains("; Secure"), "must be Secure, got: {cookie}");
    assert!(cookie.contains("Path=/;"), "must be Path=/, got: {cookie}");
    assert!(
        !cookie.to_ascii_lowercase().contains("domain="),
        "must not set Domain, got: {cookie}"
    );

    // The callback must read that same name.
    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &started.cookie_header(),
        ))
        .await
        .unwrap();

    assert!(
        response.status().is_redirection(),
        "the callback must accept the host-prefixed cookie, got {}",
        response.status()
    );
    assert_eq!(
        location_of(&response),
        "https://app.ownpulse.health/?auth=success"
    );

    let cookies = set_cookies(&response);
    assert!(
        cookies
            .iter()
            .any(|c| c.starts_with("__Host-access_token=") && c.contains("; Secure")),
        "the access token cookie must be host-prefixed and Secure too, got: {cookies:?}"
    );
    // The clear must match the cookie it is clearing in both name and path,
    // or the browser keeps the original.
    let cleared = cookies
        .iter()
        .find(|c| c.starts_with("__Host-oauth_state=;"))
        .expect("the host-prefixed state cookie must be cleared, got: {cookies:?}");
    assert!(cleared.contains("Path=/;"), "clear must be Path=/");
    assert!(cleared.contains("Max-Age=0"), "clear must expire");
    assert!(cleared.contains("; Secure"), "clear must be Secure");
}

/// The bare cookie name a plain-HTTP origin uses is not accepted when the
/// origin is HTTPS — otherwise a sibling subdomain could set the unprefixed
/// name and be believed.
#[tokio::test]
async fn test_google_callback_on_https_rejects_unprefixed_state_cookie() {
    let test_app = common::setup().await;
    let mut config = google_config("http://127.0.0.1:0");
    config.web_origin = "https://app.ownpulse.health".to_string();
    let app = google_app(&test_app.pool, config);

    let (started, _) = start_google_login(&app, "", None).await;

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &format!("oauth_state={}", started.state_cookie),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "an unprefixed state cookie must not satisfy an HTTPS-origin callback"
    );
}

/// A stray `code_verifier` is ignored: this endpoint has no PKCE branch, and
/// the parameter must not reopen one by skipping the state checks.
#[tokio::test]
async fn test_google_callback_code_verifier_does_not_bypass_state_checks() {
    let test_app = common::setup().await;
    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));

    let (started, _) = start_google_login(&app, "?platform=ios", None).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/v1/auth/google/callback?code=test-auth-code&state={}&code_verifier=dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
                    started.state
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "code_verifier must not substitute for the state cookie"
    );

    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM login_oauth_states WHERE state = $1")
            .bind(uuid::Uuid::parse_str(&started.state).unwrap())
            .fetch_one(&test_app.pool)
            .await
            .unwrap();
    assert_eq!(remaining, 1, "the row must survive a rejected callback");
}

/// Cookie present, `state` query parameter absent.
#[tokio::test]
async fn test_google_callback_missing_state_parameter_returns_400() {
    let test_app = common::setup().await;
    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));

    let (started, _) = start_google_login(&app, "", None).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/google/callback?code=test-auth-code")
                .header("cookie", started.cookie_header())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "a callback with no state parameter should return 400"
    );
}

/// Cookie and parameter agree, but the value cannot be a state we issued.
#[tokio::test]
async fn test_google_callback_non_uuid_state_returns_400() {
    let test_app = common::setup().await;
    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));

    let response = app
        .oneshot(google_callback_request(
            "not-a-uuid",
            "oauth_state=not-a-uuid",
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "a non-UUID state should return 400 before any lookup"
    );
}

/// A failure storing the state must send a native caller to the custom
/// scheme; a web page would leave `ASWebAuthenticationSession` waiting for a
/// callback that never comes.
#[tokio::test]
async fn test_google_login_state_store_failure_redirects_per_platform() {
    let test_app = common::setup().await;
    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));

    // Simulate the dependency being unavailable.
    sqlx::query("DROP TABLE login_oauth_states")
        .execute(&test_app.pool)
        .await
        .unwrap();

    for (query, expected) in [
        ("?platform=ios", "ownpulse://auth?error=server_error"),
        ("", "http://localhost:5173/login?error=server_error"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/v1/auth/google/login{query}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(
            response.status().is_redirection(),
            "a storage failure should redirect, not 500: got {}",
            response.status()
        );
        assert_eq!(location_of(&response), expected, "query: {query:?}");
    }
}

/// An over-long invite code is dropped at initiation (the column caps at 64)
/// rather than failing the flow.
#[tokio::test]
async fn test_google_login_drops_oversized_invite_code() {
    let test_app = common::setup().await;
    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));

    let long_code = "a".repeat(65);
    let (started, _) = start_google_login(&app, &format!("?invite_code={long_code}"), None).await;

    let stored: Option<String> =
        sqlx::query_scalar("SELECT invite_code FROM login_oauth_states WHERE state = $1")
            .bind(uuid::Uuid::parse_str(&started.state).unwrap())
            .fetch_one(&test_app.pool)
            .await
            .unwrap();
    assert_eq!(stored, None, "an oversized invite code must not be stored");
}

#[tokio::test]
async fn test_google_callback_web_redirects_with_cookies() {
    let test_app = common::setup().await;
    let mock_server = setup_google_mock("google-456", "webuser@example.com").await;
    let app = google_app(&test_app.pool, google_config(&mock_server.uri()));

    let (started, _) = start_google_login(&app, "", None).await;

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &started.cookie_header(),
        ))
        .await
        .unwrap();

    assert!(response.status().is_redirection());

    let location = location_of(&response);
    // Web redirect should NOT contain tokens in the URL
    assert!(
        location.starts_with("http://localhost:5173/?auth=success"),
        "expected web origin redirect without tokens, got: {location}"
    );
    assert!(
        !location.contains("token="),
        "redirect URL should NOT contain tokens"
    );

    let cookies = set_cookies(&response);
    assert!(
        cookies.iter().any(|c| c.starts_with("access_token=")),
        "web redirect should set access_token cookie, got: {cookies:?}"
    );
    assert!(
        cookies.iter().any(|c| c.starts_with("refresh_token=")),
        "web redirect should set refresh_token cookie, got: {cookies:?}"
    );
    assert!(
        cookies
            .iter()
            .any(|c| c.starts_with("oauth_state=;") && c.contains("Max-Age=0")),
        "the state cookie should be cleared, got: {cookies:?}"
    );
}

/// A flow started with `?platform=ios` ends at the custom URI scheme.
#[tokio::test]
async fn test_google_callback_ios_redirects_to_custom_scheme() {
    let test_app = common::setup().await;
    let mock_server = setup_google_mock("google-123", "iosuser@example.com").await;
    let app = google_app(&test_app.pool, google_config(&mock_server.uri()));

    let (started, _) = start_google_login(&app, "?platform=ios", None).await;

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &started.cookie_header(),
        ))
        .await
        .unwrap();

    assert!(
        response.status().is_redirection(),
        "expected redirect, got {}",
        response.status()
    );

    let location = location_of(&response);
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

/// The attack the server-side row alone does not stop: the attacker runs
/// their own initiation (this endpoint is unauthenticated), completes Google
/// consent, and navigates the victim to the callback with that genuine
/// state. The victim's browser holds no matching state cookie, so the
/// callback must refuse — otherwise the victim lands in the attacker's
/// account.
#[tokio::test]
async fn test_google_callback_rejects_attacker_initiated_state_without_cookie() {
    let test_app = common::setup().await;
    // The token URL is unroutable: the handler must reject before exchanging.
    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));

    let (attacker, _) = start_google_login(&app, "", None).await;

    // Victim's browser: the attacker's state in the URL, no state cookie.
    let response = app
        .clone()
        .oneshot(google_callback_request(&attacker.state, ""))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "a genuine state presented without the matching browser cookie must be rejected"
    );

    // The row is still there — refusal happens before it is consumed.
    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM login_oauth_states WHERE state = $1")
            .bind(uuid::Uuid::parse_str(&attacker.state).unwrap())
            .fetch_one(&test_app.pool)
            .await
            .unwrap();
    assert_eq!(remaining, 1);
}

#[tokio::test]
async fn test_google_callback_rejects_mismatched_csrf_state() {
    let test_app = common::setup().await;
    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));

    let (started, _) = start_google_login(&app, "", None).await;

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &format!("oauth_state={}", uuid::Uuid::new_v4()),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "mismatched CSRF state should return 400"
    );
}

/// A cookie and parameter that agree but correspond to no row — a guessed or
/// long-expired state — is rejected too.
#[tokio::test]
async fn test_google_callback_rejects_state_without_row() {
    let test_app = common::setup().await;
    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));

    let forged = uuid::Uuid::new_v4().to_string();
    let response = app
        .oneshot(google_callback_request(
            &forged,
            &format!("oauth_state={forged}"),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "a state with no server-side row should return 400"
    );
}

/// An expired row is not honored, even with the matching cookie.
#[tokio::test]
async fn test_google_callback_rejects_expired_state_row() {
    let test_app = common::setup().await;
    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));

    let (started, _) = start_google_login(&app, "", None).await;
    sqlx::query(
        "UPDATE login_oauth_states SET created_at = now() - interval '11 minutes' WHERE state = $1",
    )
    .bind(uuid::Uuid::parse_str(&started.state).unwrap())
    .execute(&test_app.pool)
    .await
    .unwrap();

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &started.cookie_header(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400, "an expired state should return 400");
}

/// Rows are single-use: replaying a completed callback fails.
#[tokio::test]
async fn test_google_callback_state_is_single_use() {
    let test_app = common::setup().await;
    let mock_server = setup_google_mock("google-replay", "replay@example.com").await;
    let app = google_app(&test_app.pool, google_config(&mock_server.uri()));

    let (started, _) = start_google_login(&app, "", None).await;

    let first = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &started.cookie_header(),
        ))
        .await
        .unwrap();
    assert!(first.status().is_redirection());

    let second = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &started.cookie_header(),
        ))
        .await
        .unwrap();
    assert_eq!(
        second.status(),
        400,
        "replaying a consumed state should return 400"
    );
}

/// A sibling subdomain can set `oauth_platform=ios`; it must not divert a web
/// login into the native branch, which would put both tokens in a
/// `ownpulse://` URL.
#[tokio::test]
async fn test_google_callback_injected_platform_cookie_does_not_divert_to_native() {
    let test_app = common::setup().await;
    let mock_server = setup_google_mock("google-platform-inject", "platform@example.com").await;
    let app = google_app(&test_app.pool, google_config(&mock_server.uri()));

    let (started, _) = start_google_login(&app, "", None).await;

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &format!("{}; oauth_platform=ios", started.cookie_header()),
        ))
        .await
        .unwrap();

    let location = location_of(&response);
    assert!(
        location.starts_with("http://localhost:5173/?auth=success"),
        "injected oauth_platform cookie must not select the native branch, got: {location}"
    );
    assert!(
        set_cookies(&response)
            .iter()
            .any(|c| c.starts_with("access_token=")),
        "the web branch should still set cookies"
    );
}

/// An injected `invite_code` cookie must not be spent on the victim's
/// registration — the code comes from the row only.
#[tokio::test]
async fn test_google_callback_injected_invite_cookie_is_ignored() {
    let test_app = common::setup().await;
    let inviter = insert_test_user(&test_app.pool, "inviter@example.com", "inviterpass").await;
    let code = "attackercode1";
    sqlx::query("INSERT INTO invite_codes (created_by, code) VALUES ($1, $2)")
        .bind(inviter)
        .bind(code)
        .execute(&test_app.pool)
        .await
        .unwrap();

    let mock_server = setup_google_mock("google-invite-inject", "invitee@example.com").await;
    let mut config = google_config(&mock_server.uri());
    config.require_invite = true;
    let app = google_app(&test_app.pool, config);

    // Initiated without an invite code — the attacker supplies one by cookie.
    let (started, _) = start_google_login(&app, "", None).await;

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &format!("{}; invite_code={code}", started.cookie_header()),
        ))
        .await
        .unwrap();

    let location = location_of(&response);
    assert!(
        location.contains("/register?error=invite_required"),
        "an invite_code cookie must not satisfy the invite requirement, got: {location}"
    );

    let use_count: i32 = sqlx::query_scalar("SELECT use_count FROM invite_codes WHERE code = $1")
        .bind(code)
        .fetch_one(&test_app.pool)
        .await
        .unwrap();
    assert_eq!(use_count, 0, "the injected invite must not be burned");
}

/// The invite code supplied at initiation is what registers the new user.
#[tokio::test]
async fn test_google_callback_uses_invite_code_from_initiation() {
    let test_app = common::setup().await;
    let inviter = insert_test_user(&test_app.pool, "hostess@example.com", "inviterpass").await;
    let code = "goodcode123";
    sqlx::query("INSERT INTO invite_codes (created_by, code) VALUES ($1, $2)")
        .bind(inviter)
        .bind(code)
        .execute(&test_app.pool)
        .await
        .unwrap();

    let mock_server = setup_google_mock("google-invite-good", "newjoiner@example.com").await;
    let mut config = google_config(&mock_server.uri());
    config.require_invite = true;
    let app = google_app(&test_app.pool, config);

    let (started, _) = start_google_login(&app, &format!("?invite_code={code}"), None).await;

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &started.cookie_header(),
        ))
        .await
        .unwrap();

    let location = location_of(&response);
    assert!(
        location.starts_with("http://localhost:5173/?auth=success"),
        "expected successful registration redirect, got: {location}"
    );

    let use_count: i32 = sqlx::query_scalar("SELECT use_count FROM invite_codes WHERE code = $1")
        .bind(code)
        .fetch_one(&test_app.pool)
        .await
        .unwrap();
    assert_eq!(use_count, 1, "the invite from initiation should be claimed");
}

/// An invite code that does not resolve ends in a redirect, not a JSON 400:
/// every path out of the callback is a browser navigation. The error names
/// the unusable code rather than reusing the missing-code message, so the
/// web app can tell the user which of the two happened.
#[rstest::rstest]
#[case(
    "?invite_code=nosuchcode1",
    "http://localhost:5173/register?error=invite_invalid"
)]
#[case(
    "?platform=ios&invite_code=nosuchcode1",
    "ownpulse://auth?error=invite_invalid"
)]
#[tokio::test]
async fn test_google_callback_unusable_invite_code_redirects(
    #[case] login_query: &str,
    #[case] expected: &str,
) {
    let test_app = common::setup().await;
    // A user must exist, or the first-user bootstrap waives the invite.
    insert_test_user(&test_app.pool, "existing@example.com", "existingpass").await;

    let mock_server = setup_google_mock("google-bad-invite", "badinvite@example.com").await;
    let mut config = google_config(&mock_server.uri());
    config.require_invite = true;
    let app = google_app(&test_app.pool, config);

    // Well-formed but never issued, so it passes the initiation filter and
    // fails at claim time.
    let (started, _) = start_google_login(&app, login_query, None).await;

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &started.cookie_header(),
        ))
        .await
        .unwrap();

    assert!(
        response.status().is_redirection(),
        "an unusable invite code should redirect, got {}",
        response.status()
    );
    assert_eq!(location_of(&response), expected);

    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE email = $1")
        .bind("badinvite@example.com")
        .fetch_one(&test_app.pool)
        .await
        .unwrap();
    assert_eq!(users, 0, "no account should be created");
}

/// A disabled user signing in with Google gets the export/delete access
/// token, delivered the same way a healthy sign-in is — a redirect per
/// platform, never a JSON body rendered into a browser navigation.
#[rstest::rstest]
#[case("", false)]
#[case("?platform=ios", true)]
#[tokio::test]
async fn test_google_callback_disabled_user_redirects_with_access_token_only(
    #[case] platform_query: &str,
    #[case] is_native: bool,
) {
    let test_app = common::setup().await;
    let email = "disabled-google@example.com";
    let user_id = insert_test_user(&test_app.pool, email, "disabledpass").await;
    sqlx::query("INSERT INTO user_auth_methods (user_id, provider, provider_subject, email) VALUES ($1, 'google', $2, $3)")
        .bind(user_id)
        .bind("google-disabled-login")
        .bind(email)
        .execute(&test_app.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET status = 'disabled' WHERE id = $1")
        .bind(user_id)
        .execute(&test_app.pool)
        .await
        .unwrap();

    let mock_server = setup_google_mock("google-disabled-login", email).await;
    let app = google_app(&test_app.pool, google_config(&mock_server.uri()));

    let (started, _) = start_google_login(&app, platform_query, None).await;

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &started.cookie_header(),
        ))
        .await
        .unwrap();

    assert!(
        response.status().is_redirection(),
        "expected a redirect, got {}",
        response.status()
    );
    let location = location_of(&response);
    let cookies = set_cookies(&response);

    if is_native {
        assert!(
            location.starts_with("ownpulse://auth#token="),
            "expected the custom scheme with an access token, got: {location}"
        );
        assert!(
            !location.contains("refresh_token="),
            "a disabled user must not receive a refresh token, got: {location}"
        );
    } else {
        // The token rides in the fragment, not a cookie: no route
        // authenticates from the access cookie, so a cookie would hand the
        // client nothing it could use.
        assert!(
            location.starts_with("http://localhost:5173/?auth=disabled#token="),
            "expected the token in the fragment, got: {location}"
        );
        assert!(
            !location.contains("refresh_token="),
            "a disabled user must not receive a refresh token, got: {location}"
        );
        assert!(
            !cookies.iter().any(|c| c.starts_with("refresh_token=")),
            "a disabled user must not receive a refresh token, got: {cookies:?}"
        );
    }

    let refresh_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM refresh_tokens WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&test_app.pool)
            .await
            .unwrap();
    assert_eq!(refresh_rows, 0, "no refresh token should be persisted");
}

/// Google's token endpoint failing must not surface as a raw 500 body with
/// upstream detail — the handler maps it to a 500 status with a generic body.
#[tokio::test]
async fn test_google_callback_token_exchange_failure_is_not_a_raw_upstream_error() {
    let test_app = common::setup().await;
    let mock_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/token"))
        .respond_with(wiremock::ResponseTemplate::new(500).set_body_string("upstream exploded"))
        .mount(&mock_server)
        .await;

    let app = google_app(&test_app.pool, google_config(&mock_server.uri()));
    let (started, _) = start_google_login(&app, "", None).await;

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &started.cookie_header(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 500);
    let body = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(
        !body.contains("upstream exploded"),
        "upstream response bodies must not be echoed to the client, got: {body}"
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

/// A rotated token presented within the grace window gets its own
/// successor — the multi-tab race must not 401 (and log out) the loser.
#[tokio::test]
async fn test_rotated_token_within_grace_gets_own_successor() {
    let test_app = common::setup().await;
    insert_test_user(&test_app.pool, "grace@example.com", "replaytest").await;

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
    let shared_token = extract_refresh_cookie(&login_response);

    // Tab A rotates the shared token.
    let tab_a = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={shared_token}"),
        ))
        .await
        .unwrap();
    assert_eq!(tab_a.status(), 200);
    let token_a = extract_refresh_cookie(&tab_a);
    assert_ne!(shared_token, token_a, "token should have rotated");

    // Tab B presents the same (now rotated) token within the grace window.
    let tab_b = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={shared_token}"),
        ))
        .await
        .unwrap();
    assert_eq!(
        tab_b.status(),
        200,
        "a concurrent refresh within the grace window must not 401"
    );
    let token_b = extract_refresh_cookie(&tab_b);
    assert_eq!(
        token_a, token_b,
        "grace replays return the SAME successor — a fork would let a thief \
         hold an undetectable parallel session"
    );

    // Every grace replay keeps returning that successor.
    let tab_c = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={shared_token}"),
        ))
        .await
        .unwrap();
    assert_eq!(tab_c.status(), 200);
    assert_eq!(extract_refresh_cookie(&tab_c), token_a);

    // The shared successor is usable and rotates normally.
    let next = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={token_a}"),
        ))
        .await
        .unwrap();
    assert_eq!(next.status(), 200, "successor token must be usable");
}

/// Rotation opportunistically sweeps the family's dead rows: rotated past
/// grace, or expired. Active rows must survive the sweep.
#[tokio::test]
async fn test_rotation_sweeps_stale_family_rows() {
    let test_app = common::setup().await;
    let user_id = insert_test_user(&test_app.pool, "sweep@example.com", "replaytest").await;

    let login_response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": "sweep@example.com", "password": "replaytest"}),
        ))
        .await
        .unwrap();
    assert_eq!(login_response.status(), 200);
    let t0 = extract_refresh_cookie(&login_response);

    // Rotate T0 → T1, then age T0 past the grace window.
    let r1 = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={t0}"),
        ))
        .await
        .unwrap();
    assert_eq!(r1.status(), 200);
    let t1 = extract_refresh_cookie(&r1);

    sqlx::query(
        "UPDATE refresh_tokens SET rotated_at = rotated_at - interval '10 minutes'
         WHERE user_id = $1 AND rotated_at IS NOT NULL",
    )
    .bind(user_id)
    .execute(&test_app.pool)
    .await
    .unwrap();

    // Rotating T1 sweeps the stale T0 row and leaves the fresh rows intact.
    let r2 = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={t1}"),
        ))
        .await
        .unwrap();
    assert_eq!(r2.status(), 200);
    let t2 = extract_refresh_cookie(&r2);

    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM refresh_tokens WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&test_app.pool)
            .await
            .unwrap();
    assert_eq!(
        remaining, 2,
        "sweep removes the stale rotated row, keeping T1 (in grace) and T2 (active)"
    );

    // The surviving active token still works.
    let r3 = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={t2}"),
        ))
        .await
        .unwrap();
    assert_eq!(r3.status(), 200);
}

/// Reuse of a rotated token after the grace window is treated as theft:
/// 401, and every token in the family is revoked.
#[tokio::test]
async fn test_reuse_after_grace_revokes_family() {
    let test_app = common::setup().await;
    let user_id = insert_test_user(&test_app.pool, "reuse@example.com", "replaytest").await;

    let login_response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": "reuse@example.com", "password": "replaytest"}),
        ))
        .await
        .unwrap();
    assert_eq!(login_response.status(), 200);
    let old_token = extract_refresh_cookie(&login_response);

    let refresh_response = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={old_token}"),
        ))
        .await
        .unwrap();
    assert_eq!(refresh_response.status(), 200);
    let new_token = extract_refresh_cookie(&refresh_response);

    // Age the rotation past the grace window (no sleeping).
    sqlx::query(
        "UPDATE refresh_tokens SET rotated_at = rotated_at - interval '10 minutes'
         WHERE user_id = $1 AND rotated_at IS NOT NULL",
    )
    .bind(user_id)
    .execute(&test_app.pool)
    .await
    .unwrap();

    let reuse_response = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={old_token}"),
        ))
        .await
        .unwrap();
    assert_eq!(
        reuse_response.status(),
        401,
        "reuse after the grace window must be rejected"
    );

    // The whole family is dead: the current token is revoked too.
    let successor_response = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={new_token}"),
        ))
        .await
        .unwrap();
    assert_eq!(
        successor_response.status(),
        401,
        "family revocation must invalidate the successor token"
    );

    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM refresh_tokens WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&test_app.pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0, "no tokens should survive family revocation");
}

/// A token whose family was revoked by logout must stay dead. Covers the
/// sequential case; the concurrent race is closed by the refresh handler's
/// FOR UPDATE row lock plus the multi-pass revocation delete.
#[tokio::test]
async fn test_refresh_after_logout_does_not_resurrect_session() {
    let test_app = common::setup().await;
    let user_id = insert_test_user(&test_app.pool, "postlogout@example.com", "replaytest").await;

    let login_response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": "postlogout@example.com", "password": "replaytest"}),
        ))
        .await
        .unwrap();
    assert_eq!(login_response.status(), 200);
    let token = extract_refresh_cookie(&login_response);

    let logout_response = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/logout",
            &format!("refresh_token={token}"),
        ))
        .await
        .unwrap();
    assert_eq!(logout_response.status(), 204);

    let refresh_response = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={token}"),
        ))
        .await
        .unwrap();
    assert_eq!(refresh_response.status(), 401);

    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM refresh_tokens WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&test_app.pool)
            .await
            .unwrap();
    assert_eq!(
        remaining, 0,
        "refresh after logout must not mint new tokens"
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
    let app = google_app(&test_app.pool, google_config(&mock_server.uri()));

    let (started, _) = start_google_login(&app, "", None).await;

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &started.cookie_header(),
        ))
        .await
        .unwrap();

    assert!(
        response.status().is_redirection(),
        "expected redirect, got {}",
        response.status()
    );
    let location = location_of(&response);
    assert!(
        location.contains("/login?error=email_exists"),
        "expected email_exists error redirect, got: {location}"
    );
}

/// Email collision on an iOS-initiated flow redirects to the custom scheme.
#[tokio::test]
async fn test_google_callback_email_collision_ios_redirects_with_error() {
    let test_app = common::setup().await;
    let email = "ios-collision@example.com";
    insert_test_user(&test_app.pool, email, "existingpass").await;

    let mock_server = setup_google_mock("google-ios-collision-sub", email).await;
    let app = google_app(&test_app.pool, google_config(&mock_server.uri()));

    let (started, _) = start_google_login(&app, "?platform=ios", None).await;

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &started.cookie_header(),
        ))
        .await
        .unwrap();

    assert!(
        response.status().is_redirection(),
        "expected redirect, got {}",
        response.status()
    );
    let location = location_of(&response);
    assert!(
        location.starts_with("ownpulse://auth?error=email_exists"),
        "expected native email_exists redirect, got: {location}"
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

/// Start a link-mode flow as the holder of `access_token`.
async fn start_google_link(app: &axum::Router, access_token: &str) -> StartedLogin {
    let (started, _) = start_google_login(
        app,
        "?mode=link",
        Some(&format!("access_token={access_token}")),
    )
    .await;
    started
}

/// An authenticated user can link their Google account.
#[tokio::test]
async fn test_google_link_flow_succeeds() {
    let test_app = common::setup().await;
    let (user_id, access_token) =
        create_user_with_access_token(&test_app.pool, "linker@example.com").await;

    let mock_server = setup_google_mock("google-link-sub", "linker-google@example.com").await;
    let app = google_app(&test_app.pool, google_config(&mock_server.uri()));

    let started = start_google_link(&app, &access_token).await;

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &format!("{}; access_token={access_token}", started.cookie_header()),
        ))
        .await
        .unwrap();

    assert!(
        response.status().is_redirection(),
        "expected redirect, got {}",
        response.status()
    );
    let location = location_of(&response);
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

/// The account a Google identity attaches to is decided at initiation. An
/// access-token cookie for a different user, present at callback time (a
/// sibling subdomain can inject one), must not redirect the link.
#[tokio::test]
async fn test_google_link_binds_user_at_initiation() {
    let test_app = common::setup().await;
    let (victim_id, victim_token) =
        create_user_with_access_token(&test_app.pool, "victim@example.com").await;
    let (attacker_id, attacker_token) =
        create_user_with_access_token(&test_app.pool, "attacker@example.com").await;

    let mock_server = setup_google_mock("google-bind-sub", "victim-google@example.com").await;
    let app = google_app(&test_app.pool, google_config(&mock_server.uri()));

    // The victim starts the link flow.
    let started = start_google_link(&app, &victim_token).await;

    // The attacker's access_token is injected before the callback lands.
    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &format!("{}; access_token={attacker_token}", started.cookie_header()),
        ))
        .await
        .unwrap();

    assert!(response.status().is_redirection());

    let owner: uuid::Uuid = sqlx::query_scalar(
        "SELECT user_id FROM user_auth_methods WHERE provider = 'google' AND provider_subject = $1",
    )
    .bind("google-bind-sub")
    .fetch_one(&test_app.pool)
    .await
    .expect("the google identity should have been linked");

    assert_eq!(
        owner, victim_id,
        "the link must bind to the user who started the flow, not the injected cookie"
    );
    assert_ne!(owner, attacker_id);
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
    let app = google_app(&test_app.pool, google_config(&mock_server.uri()));

    let started = start_google_link(&app, &access_token).await;

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &format!("{}; access_token={access_token}", started.cookie_header()),
        ))
        .await
        .unwrap();

    assert!(
        response.status().is_redirection(),
        "expected redirect, got {}",
        response.status()
    );
    let location = location_of(&response);
    assert!(
        location.contains("/settings?error=already_linked"),
        "expected already_linked error redirect, got: {location}"
    );
}

/// Without an access_token cookie, link mode is refused at initiation — the
/// callback is never reached, so no state row is created.
#[tokio::test]
async fn test_google_link_unauthenticated_redirects_to_settings() {
    let test_app = common::setup().await;
    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/google/login?mode=link")
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
    let location = location_of(&response);
    assert!(
        location.contains("/settings?error=auth_required"),
        "expected auth_required error redirect, got: {location}"
    );

    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM login_oauth_states")
        .fetch_one(&test_app.pool)
        .await
        .unwrap();
    assert_eq!(rows, 0, "a refused link initiation must not store a state");
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
    let app = google_app(&test_app.pool, google_config(&mock_server.uri()));

    let started = start_google_link(&app, &access_token).await;

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &format!("{}; access_token={access_token}", started.cookie_header()),
        ))
        .await
        .unwrap();

    assert!(
        response.status().is_redirection(),
        "expected redirect, got {}",
        response.status()
    );
    let location = location_of(&response);
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

/// A user disabled between initiation and callback gets a 403.
#[tokio::test]
async fn test_google_link_disabled_user_fails() {
    let test_app = common::setup().await;
    let (user_id, access_token) =
        create_user_with_access_token(&test_app.pool, "disabled-linker@example.com").await;

    let mock_server = setup_google_mock(
        "google-disabled-link-sub",
        "disabled-linker-google@example.com",
    )
    .await;
    let app = google_app(&test_app.pool, google_config(&mock_server.uri()));

    let started = start_google_link(&app, &access_token).await;

    sqlx::query("UPDATE users SET status = 'disabled' WHERE id = $1")
        .bind(user_id)
        .execute(&test_app.pool)
        .await
        .expect("failed to disable user");

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &format!("{}; access_token={access_token}", started.cookie_header()),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        403,
        "disabled user should get 403, got {}",
        response.status()
    );
}

/// Deleting the linking user between initiation and callback cascades the
/// state row away, so the callback fails closed rather than linking the
/// Google identity to a dangling id.
#[tokio::test]
async fn test_google_link_deleted_user_state_row_is_gone() {
    let test_app = common::setup().await;
    let (user_id, access_token) =
        create_user_with_access_token(&test_app.pool, "deleted-linker@example.com").await;

    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));
    let started = start_google_link(&app, &access_token).await;

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&test_app.pool)
        .await
        .expect("failed to delete user");

    let response = app
        .clone()
        .oneshot(google_callback_request(
            &started.state,
            &format!("{}; access_token={access_token}", started.cookie_header()),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "the cascaded-away state row should be rejected, got {}",
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

/// The daily sweep deletes tokens expired past the retention margin and
/// nothing else.
#[tokio::test]
async fn test_expired_token_sweep_removes_only_expired_rows() {
    let test_app = common::setup().await;
    let user_id = insert_test_user(&test_app.pool, "sweepjob@example.com", "sweeppass").await;

    // Three rows: long-expired (swept), freshly-expired (kept — inside the
    // seven-day theft-detection margin), and valid (kept).
    sqlx::query(
        "INSERT INTO refresh_tokens (user_id, token_hash, expires_at, family_id)
         VALUES ($1, 'long-expired-hash', now() - interval '8 days', gen_random_uuid()),
                ($1, 'fresh-expired-hash', now() - interval '1 day', gen_random_uuid()),
                ($1, 'valid-hash', now() + interval '1 day', gen_random_uuid())",
    )
    .bind(user_id)
    .execute(&test_app.pool)
    .await
    .unwrap();

    let removed = api::db::refresh_tokens::delete_expired(&test_app.pool)
        .await
        .unwrap();
    assert_eq!(removed, 1);

    let mut remaining: Vec<String> =
        sqlx::query_scalar("SELECT token_hash FROM refresh_tokens WHERE user_id = $1")
            .bind(user_id)
            .fetch_all(&test_app.pool)
            .await
            .unwrap();
    remaining.sort();
    assert_eq!(
        remaining,
        vec!["fresh-expired-hash".to_string(), "valid-hash".to_string()]
    );
}

/// The same sweep clears OAuth states left behind by flows the user
/// abandoned at the provider's consent screen, in both state tables, and
/// leaves live rows alone.
#[tokio::test]
async fn test_expired_oauth_state_sweep_removes_only_expired_rows() {
    let test_app = common::setup().await;
    let user_id = insert_test_user(&test_app.pool, "statesweep@example.com", "sweeppass").await;

    let (live_connect, stale_connect) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    api::db::oauth_states::insert(&test_app.pool, live_connect, user_id, "google_calendar")
        .await
        .unwrap();
    api::db::oauth_states::insert(&test_app.pool, stale_connect, user_id, "google_calendar")
        .await
        .unwrap();

    let (live_login, stale_login) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    api::db::login_oauth_states::insert(&test_app.pool, live_login, false, None, None)
        .await
        .unwrap();
    api::db::login_oauth_states::insert(&test_app.pool, stale_login, true, None, Some(user_id))
        .await
        .unwrap();

    for (table, state) in [
        ("oauth_states", stale_connect),
        ("login_oauth_states", stale_login),
    ] {
        sqlx::query(&format!(
            "UPDATE {table} SET created_at = now() - interval '11 minutes' WHERE state = $1"
        ))
        .bind(state)
        .execute(&test_app.pool)
        .await
        .unwrap();
    }

    assert_eq!(
        api::db::oauth_states::delete_expired(&test_app.pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        api::db::login_oauth_states::delete_expired(&test_app.pool)
            .await
            .unwrap(),
        1
    );

    let connect_left: Vec<uuid::Uuid> = sqlx::query_scalar("SELECT state FROM oauth_states")
        .fetch_all(&test_app.pool)
        .await
        .unwrap();
    assert_eq!(connect_left, vec![live_connect]);
    let login_left: Vec<uuid::Uuid> = sqlx::query_scalar("SELECT state FROM login_oauth_states")
        .fetch_all(&test_app.pool)
        .await
        .unwrap();
    assert_eq!(login_left, vec![live_login]);
}

/// Constraint coverage for `login_oauth_states`: the primary key rejects a
/// reused state, the foreign key rejects an unknown link target, and the
/// check constraint rejects an invite code longer than the column allows.
#[tokio::test]
async fn test_login_oauth_state_constraints() {
    let test_app = common::setup().await;
    let user_id =
        insert_test_user(&test_app.pool, "constraints@example.com", "constraintpass").await;

    let state = uuid::Uuid::new_v4();
    api::db::login_oauth_states::insert(&test_app.pool, state, false, Some("code1"), Some(user_id))
        .await
        .expect("first insert should succeed");

    let duplicate =
        api::db::login_oauth_states::insert(&test_app.pool, state, false, None, None).await;
    assert!(
        matches!(&duplicate, Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23505")),
        "reusing a state must violate the primary key, got: {duplicate:?}"
    );

    let unknown_user = api::db::login_oauth_states::insert(
        &test_app.pool,
        uuid::Uuid::new_v4(),
        false,
        None,
        Some(uuid::Uuid::new_v4()),
    )
    .await;
    assert!(
        matches!(&unknown_user, Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23503")),
        "an unknown link_user_id must violate the foreign key, got: {unknown_user:?}"
    );

    let long_code = "a".repeat(65);
    let oversized = api::db::login_oauth_states::insert(
        &test_app.pool,
        uuid::Uuid::new_v4(),
        false,
        Some(&long_code),
        None,
    )
    .await;
    assert!(
        matches!(&oversized, Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23514")),
        "an over-long invite code must violate the check constraint, got: {oversized:?}"
    );

    // Consume returns what was stored, and only once.
    let consumed = api::db::login_oauth_states::consume(&test_app.pool, state)
        .await
        .unwrap()
        .expect("the row should still be there");
    assert!(!consumed.is_native);
    assert_eq!(consumed.invite_code.as_deref(), Some("code1"));
    assert_eq!(consumed.link_user_id, Some(user_id));
    assert!(
        api::db::login_oauth_states::consume(&test_app.pool, state)
            .await
            .unwrap()
            .is_none(),
        "a consumed row must not be returned twice"
    );
}

/// Two *live* refresh cookies is the fixation attempt: a sibling subdomain
/// injected a session of its own alongside ours, and the server cannot see
/// which cookie is host-only. It refuses rather than choosing.
#[tokio::test]
async fn test_two_live_refresh_cookies_are_rejected() {
    let test_app = common::setup().await;
    insert_test_user(&test_app.pool, "victim@example.com", "replaytest").await;
    insert_test_user(&test_app.pool, "attacker@example.com", "replaytest").await;

    let mut tokens = Vec::new();
    for email in ["victim@example.com", "attacker@example.com"] {
        let login = test_app
            .app
            .clone()
            .oneshot(post_json(
                "/api/v1/auth/login",
                &json!({"email": email, "password": "replaytest"}),
            ))
            .await
            .unwrap();
        assert_eq!(login.status(), 200);
        tokens.push(extract_refresh_cookie(&login));
    }

    let resp = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={}; refresh_token={}", tokens[1], tokens[0]),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

/// A stale or junk duplicate must not lock the user out: they cannot clear
/// a cookie another subdomain set, so counting cookies rather than live
/// sessions would strand them in a permanent re-login loop.
#[tokio::test]
async fn test_dead_duplicate_refresh_cookie_does_not_block_refresh() {
    let test_app = common::setup().await;
    insert_test_user(&test_app.pool, "stale@example.com", "replaytest").await;

    let login = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": "stale@example.com", "password": "replaytest"}),
        ))
        .await
        .unwrap();
    assert_eq!(login.status(), 200);
    let real_token = extract_refresh_cookie(&login);

    let resp = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token=long-dead-value; refresh_token={real_token}"),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        200,
        "a dead duplicate must be ignored, not treated as an attack"
    );
}

/// Logout must end the real session even when an injected cookie is sent
/// alongside it — revoking only the first-parsed token would leave the
/// user logged in.
#[tokio::test]
async fn test_logout_revokes_every_presented_refresh_cookie() {
    let test_app = common::setup().await;
    let user_id = insert_test_user(&test_app.pool, "logoutall@example.com", "replaytest").await;

    let login_response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": "logoutall@example.com", "password": "replaytest"}),
        ))
        .await
        .unwrap();
    assert_eq!(login_response.status(), 200);
    let real_token = extract_refresh_cookie(&login_response);

    let logout = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/logout",
            &format!("refresh_token=injected-value; refresh_token={real_token}"),
        ))
        .await
        .unwrap();
    assert_eq!(logout.status(), 204);

    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM refresh_tokens WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&test_app.pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0, "the real session must be revoked");
}

/// Logout must clear the access-token cookie, not just the refresh one.
/// It outlives logout by up to its expiry, and the Google link flow reads
/// it to decide which account a provider identity attaches to — so leaving
/// it behind keeps a signed-out browser able to act as that user.
#[tokio::test]
async fn test_logout_clears_the_access_token_cookie() {
    let test_app = common::setup().await;
    insert_test_user(&test_app.pool, "clearcookie@example.com", "replaytest").await;

    let login = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": "clearcookie@example.com", "password": "replaytest"}),
        ))
        .await
        .unwrap();
    assert_eq!(login.status(), 200);
    let refresh_token = extract_refresh_cookie(&login);

    let logout = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/logout",
            &format!("refresh_token={refresh_token}"),
        ))
        .await
        .unwrap();
    assert_eq!(logout.status(), 204);

    let cleared: Vec<String> = logout
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(str::to_string)
        .collect();

    assert!(
        cleared.iter().any(|c| c.starts_with("refresh_token=;")),
        "refresh cookie must be cleared, got: {cleared:?}"
    );
    assert!(
        cleared
            .iter()
            .any(|c| c.starts_with("access_token=;") || c.starts_with("__Host-access_token=;")),
        "access-token cookie must be cleared, got: {cleared:?}"
    );
}

/// Declining Google's consent screen sends `error` and no `code`. That must
/// reach the handler and redirect — making `code` mandatory would fail query
/// extraction first and render a raw 400 body, which inside an iOS auth
/// session is a hang.
#[rstest::rstest]
#[case("", "http://localhost:5173/login?error=google_declined")]
#[case("?platform=ios", "ownpulse://auth?error=access_denied")]
#[tokio::test]
async fn test_google_callback_declined_consent_redirects(
    #[case] login_query: &str,
    #[case] expected: &str,
) {
    let test_app = common::setup().await;
    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));
    let (started, _) = start_google_login(&app, login_query, None).await;

    let request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/v1/auth/google/callback?error=access_denied&state={}",
            started.state
        ))
        .header("cookie", started.cookie_header())
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    assert!(response.status().is_redirection());
    assert_eq!(location_of(&response), expected);
}

/// Link mode's exits are all web URLs, so pairing it with a native platform
/// would strand an iOS auth session on a page it cannot complete.
#[tokio::test]
async fn test_google_login_rejects_link_mode_from_a_native_client() {
    let test_app = common::setup().await;
    let app = google_app(&test_app.pool, google_config("http://127.0.0.1:0"));

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/auth/google/login?mode=link&platform=ios")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 400);
}
