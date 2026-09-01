// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Integration tests for the Google Calendar OAuth 2.0 connect flow and the
//! `integrations::google::refresh_access_token` helper it shares with the
//! sync job.

use axum::body::Body;
use http::Request;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common;

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn get_with_auth(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

fn location(response: &axum::response::Response) -> String {
    response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string()
}

/// Pull `?state=` out of an `auth_url` with proper query-string parsing
/// (`reqwest::Url`, a re-export of `url::Url`) rather than string-splitting,
/// which would be fragile against parameter reordering or `&` inside an
/// encoded value.
fn extract_state_param(auth_url: &str) -> String {
    reqwest::Url::parse(auth_url)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "state")
        .expect("auth_url should contain a state param")
        .1
        .to_string()
}

async fn login(app: &common::TestApp, token: &str) -> axum::response::Response {
    app.app
        .clone()
        .oneshot(get_with_auth("/api/v1/auth/google-calendar/login", token))
        .await
        .unwrap()
}

async fn callback(app: &common::TestApp, query: &str) -> axum::response::Response {
    app.app
        .clone()
        .oneshot(get(&format!(
            "/api/v1/auth/google-calendar/callback?{query}"
        )))
        .await
        .unwrap()
}

#[tokio::test]
async fn google_calendar_login_returns_auth_url_json() {
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let response = login(&app, &token).await;
    assert_eq!(response.status(), 200);

    let body = body_json(response).await;
    let auth_url = body["auth_url"].as_str().unwrap();
    assert!(auth_url.starts_with("https://accounts.google.com"));
    assert!(auth_url.contains("client_id=test-google-id"));
    assert!(auth_url.contains("access_type=offline"));
    assert!(auth_url.contains("prompt=consent"));
    assert!(auth_url.contains("calendar.readonly"));

    // A valid, parseable state param, since the callback round trip below
    // depends on being able to extract and reuse it.
    let state = extract_state_param(auth_url);
    assert!(uuid::Uuid::parse_str(&state).is_ok());
}

#[tokio::test]
async fn google_calendar_login_requires_auth() {
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let response = app
        .app
        .clone()
        .oneshot(get("/api/v1/auth/google-calendar/login"))
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

/// Full round trip: JSON login (Bearer auth, no cookies at all) records a
/// server-side state row; the callback that follows — reached with no
/// `Authorization` header, as a browser navigation genuinely can't send one
/// — consumes that row to recover identity and stores the token under the
/// *login* user. Also proves the state row is single-use: replaying the
/// same callback a second time is rejected.
#[tokio::test]
async fn google_calendar_full_round_trip_binds_callback_to_login_user() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "gcal-access-token",
            "refresh_token": "gcal-refresh-token",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
        cfg.google_token_url = format!("{}/token", mock_server.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;

    let login_response = login(&app, &token).await;
    assert_eq!(login_response.status(), 200);
    assert!(
        login_response.headers().get("set-cookie").is_none(),
        "no cookie should be set anywhere in this flow"
    );
    let auth_url = body_json(login_response).await["auth_url"]
        .as_str()
        .unwrap()
        .to_string();
    let state = extract_state_param(&auth_url);

    let callback_response = callback(&app, &format!("code=test-code&state={state}")).await;
    assert_eq!(
        location(&callback_response),
        "http://localhost:5173/sources?connected=google_calendar"
    );

    let row = sqlx::query_as::<_, (uuid::Uuid, String)>(
        "SELECT user_id, access_token FROM integration_tokens WHERE source = 'google_calendar'",
    )
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(row.0, user_id);
    assert_ne!(row.1, "gcal-access-token");
    assert!(row.1.starts_with("v1:"), "token should be encrypted");

    // State reuse: the row was deleted on first use, so a replay (e.g. the
    // user double-clicking back, or an attacker replaying the redirect URL)
    // finds nothing and is rejected rather than reconnecting silently.
    let replay_response = callback(&app, &format!("code=test-code&state={state}")).await;
    assert_eq!(
        location(&replay_response),
        "http://localhost:5173/sources?error=state_invalid"
    );
}

/// Two concurrent connect attempts (e.g. two browser tabs) must not cross
/// wires — the callback for user B's state must never attribute the token
/// to user A, even though both rows exist in `oauth_states` at once.
#[tokio::test]
async fn google_calendar_callback_cross_user_isolation() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "gcal-access-token-b",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
        cfg.google_token_url = format!("{}/token", mock_server.uri());
    })
    .await;

    let (_user_a_id, token_a) = common::create_test_user(&app).await;
    let (user_b_id, token_b) = common::create_test_user(&app).await;

    let auth_url_a = body_json(login(&app, &token_a).await).await["auth_url"]
        .as_str()
        .unwrap()
        .to_string();
    let _state_a = extract_state_param(&auth_url_a);

    let auth_url_b = body_json(login(&app, &token_b).await).await["auth_url"]
        .as_str()
        .unwrap()
        .to_string();
    let state_b = extract_state_param(&auth_url_b);

    let response = callback(&app, &format!("code=test-code&state={state_b}")).await;
    assert_eq!(
        location(&response),
        "http://localhost:5173/sources?connected=google_calendar"
    );

    let row = sqlx::query_as::<_, (uuid::Uuid,)>(
        "SELECT user_id FROM integration_tokens WHERE source = 'google_calendar'",
    )
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        row.0, user_b_id,
        "token must be attributed to user B, not A"
    );
}

#[tokio::test]
async fn google_calendar_callback_deny_path_redirects_and_consumes_state() {
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;
    let auth_url = body_json(login(&app, &token).await).await["auth_url"]
        .as_str()
        .unwrap()
        .to_string();
    let state = extract_state_param(&auth_url);

    let response = callback(&app, &format!("error=access_denied&state={state}")).await;
    assert_eq!(
        location(&response),
        "http://localhost:5173/sources?error=access_denied"
    );

    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_states WHERE state = $1")
        .bind(uuid::Uuid::parse_str(&state).unwrap())
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(
        remaining, 0,
        "state row should be consumed on the deny path too"
    );
}

#[tokio::test]
async fn google_calendar_callback_rejects_unknown_state() {
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let response = callback(
        &app,
        &format!("code=test-code&state={}", uuid::Uuid::new_v4()),
    )
    .await;

    assert_eq!(
        location(&response),
        "http://localhost:5173/sources?error=state_invalid"
    );
}

#[tokio::test]
async fn google_calendar_callback_rejects_malformed_state() {
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let response = callback(&app, "code=test-code&state=not-a-uuid").await;

    assert_eq!(
        location(&response),
        "http://localhost:5173/sources?error=state_invalid"
    );
}

#[tokio::test]
async fn google_calendar_callback_rejects_expired_state() {
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let (user_id, _token) = common::create_test_user(&app).await;
    let state = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO oauth_states (state, user_id, provider, created_at) \
         VALUES ($1, $2, 'google_calendar', now() - interval '11 minutes')",
    )
    .bind(state)
    .bind(user_id)
    .execute(&app.pool)
    .await
    .unwrap();

    let response = callback(&app, &format!("code=test-code&state={state}")).await;

    assert_eq!(
        location(&response),
        "http://localhost:5173/sources?error=state_invalid"
    );

    // Still single-use even when expired — an expired row isn't left around
    // for a later, also-doomed retry to trip over.
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_states WHERE state = $1")
        .bind(state)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn google_calendar_callback_rejects_missing_code_without_error() {
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;
    let auth_url = body_json(login(&app, &token).await).await["auth_url"]
        .as_str()
        .unwrap()
        .to_string();
    let state = extract_state_param(&auth_url);

    let response = callback(&app, &format!("state={state}")).await;

    assert_eq!(
        location(&response),
        "http://localhost:5173/sources?error=missing_code"
    );
}

#[tokio::test]
async fn google_calendar_callback_exchange_failure_redirects() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": "invalid_grant"
        })))
        .mount(&mock_server)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
        cfg.google_token_url = format!("{}/token", mock_server.uri());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;
    let auth_url = body_json(login(&app, &token).await).await["auth_url"]
        .as_str()
        .unwrap()
        .to_string();
    let state = extract_state_param(&auth_url);

    let response = callback(&app, &format!("code=expired-code&state={state}")).await;

    assert_eq!(
        location(&response),
        "http://localhost:5173/sources?error=exchange_failed"
    );
}

#[tokio::test]
async fn list_integrations_includes_google_calendar_once_connected() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;
    let key = api::crypto::parse_encryption_key(
        "0000000000000000000000000000000000000000000000000000000000000000",
    )
    .unwrap();
    api::db::integration_tokens::upsert(
        &app.pool,
        user_id,
        "google_calendar",
        "gcal-access",
        Some("gcal-refresh"),
        None,
        &key,
    )
    .await
    .unwrap();

    let response = app
        .app
        .clone()
        .oneshot(get_with_auth("/api/v1/integrations", &token))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = body_json(response).await;
    let sources: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["source"].as_str().unwrap())
        .collect();
    assert!(sources.contains(&"google_calendar"));
}

// ── `integrations::google::refresh_access_token` ─────────────────────────

#[tokio::test]
async fn google_refresh_access_token_succeeds() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "refreshed-access",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    let result = api::integrations::google::refresh_access_token(
        &reqwest::Client::new(),
        "client-id",
        "client-secret",
        "refresh-token",
        &format!("{}/token", mock_server.uri()),
    )
    .await;

    let tokens = result.unwrap();
    assert_eq!(tokens.access_token, "refreshed-access");
    assert_eq!(tokens.expires_in.unwrap(), 3600);
}

#[tokio::test]
async fn google_refresh_access_token_handles_error_response() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": "invalid_grant",
            "error_description": "Token has been expired or revoked."
        })))
        .mount(&mock_server)
        .await;

    let result = api::integrations::google::refresh_access_token(
        &reqwest::Client::new(),
        "client-id",
        "client-secret",
        "revoked-refresh-token",
        &format!("{}/token", mock_server.uri()),
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn google_refresh_access_token_handles_malformed_response() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&mock_server)
        .await;

    let result = api::integrations::google::refresh_access_token(
        &reqwest::Client::new(),
        "client-id",
        "client-secret",
        "refresh-token",
        &format!("{}/token", mock_server.uri()),
    )
    .await;

    assert!(result.is_err());
}
