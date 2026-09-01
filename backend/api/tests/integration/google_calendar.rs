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

fn get_with_cookies(uri: &str, cookies: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("cookie", cookies)
        .body(Body::empty())
        .unwrap()
}

/// The callback authenticates from `google_calendar_oauth_user` (an access
/// token, set by `google_calendar_login`), not an `Authorization` header —
/// it's a full-page GET the browser follows after Google redirects back, so
/// it can't carry one. Build the cookie header a real browser would send:
/// both the CSRF state cookie and the user-binding cookie.
fn callback_cookies(csrf_state: &str, user_token: &str) -> String {
    format!("google_calendar_oauth_state={csrf_state}; google_calendar_oauth_user={user_token}")
}

#[tokio::test]
async fn google_calendar_login_redirects_with_calendar_scope() {
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(get_with_auth("/api/v1/auth/google-calendar/login", &token))
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
        .unwrap()
        .to_str()
        .unwrap();
    assert!(location.contains("accounts.google.com"));
    assert!(location.contains("client_id=test-google-id"));
    assert!(location.contains("access_type=offline"));
    assert!(location.contains("prompt=consent"));
    assert!(location.contains("calendar.readonly"));

    let set_cookie = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect::<Vec<_>>()
        .join("; ");
    assert!(set_cookie.contains("google_calendar_oauth_state="));
}

#[tokio::test]
async fn google_calendar_login_accepts_query_token_for_browser_navigation() {
    // The web Sources page's Connect control is a plain `<a href>`, which
    // cannot attach an Authorization header — this must work the same way
    // the header-based request above does.
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/auth/google-calendar/login?token={token}"))
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
        .unwrap()
        .to_str()
        .unwrap();
    assert!(location.contains("accounts.google.com"));
}

#[tokio::test]
async fn google_calendar_login_rejects_bad_query_token() {
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let response = app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/google-calendar/login?token=not-a-valid-jwt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn google_calendar_login_rejects_absent_token() {
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let response = app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/google-calendar/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn google_calendar_callback_exchanges_code_and_stores_tokens() {
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

    let csrf_state = "test-csrf-state";
    let response = app
        .app
        .clone()
        .oneshot(get_with_cookies(
            &format!("/api/v1/auth/google-calendar/callback?code=test-code&state={csrf_state}"),
            &callback_cookies(csrf_state, &token),
        ))
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
        .unwrap()
        .to_str()
        .unwrap();
    assert!(location.contains("connected=google_calendar"));

    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT source, access_token FROM integration_tokens \
         WHERE user_id = $1 AND source = 'google_calendar'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    assert_eq!(row.0, "google_calendar");
    assert_ne!(row.1, "gcal-access-token");
    assert!(
        row.1.starts_with("v1:"),
        "encrypted token should have v1: prefix"
    );
}

/// Full browser round trip: `?token=` login (no header available) sets both
/// cookies, and the callback that follows authenticates from them alone —
/// proves the two legs are wired together correctly, not just each one in
/// isolation against a hand-built cookie header.
#[tokio::test]
async fn google_calendar_full_browser_flow_binds_callback_to_the_login_user() {
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

    let login_response = app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/auth/google-calendar/login?token={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(login_response.status().is_redirection());

    let auth_url = login_response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let csrf_state = auth_url
        .split("&state=")
        .nth(1)
        .expect("auth url should contain state param");

    let cookies = login_response
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string())
        .collect::<Vec<_>>()
        .join("; ");
    assert!(cookies.contains("google_calendar_oauth_state="));
    assert!(cookies.contains("google_calendar_oauth_user="));

    let callback_response = app
        .app
        .clone()
        .oneshot(get_with_cookies(
            &format!("/api/v1/auth/google-calendar/callback?code=test-code&state={csrf_state}"),
            &cookies,
        ))
        .await
        .unwrap();

    assert!(
        callback_response.status().is_redirection(),
        "expected redirect, got {}",
        callback_response.status()
    );

    let row = sqlx::query_as::<_, (uuid::Uuid,)>(
        "SELECT user_id FROM integration_tokens WHERE source = 'google_calendar'",
    )
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(row.0, user_id);
}

#[tokio::test]
async fn google_calendar_callback_rejects_state_mismatch() {
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(get_with_cookies(
            "/api/v1/auth/google-calendar/callback?code=test-code&state=wrong-state",
            &callback_cookies("correct-state", &token),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
    let body = body_json(response).await;
    assert!(body["error"].as_str().unwrap().contains("state mismatch"));
}

#[tokio::test]
async fn google_calendar_callback_rejects_missing_state_cookie() {
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(get_with_cookies(
            "/api/v1/auth/google-calendar/callback?code=test-code&state=some-state",
            &format!("google_calendar_oauth_user={token}"),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn google_calendar_callback_rejects_missing_user_cookie() {
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let response = app
        .app
        .clone()
        .oneshot(get_with_cookies(
            "/api/v1/auth/google-calendar/callback?code=test-code&state=some-state",
            "google_calendar_oauth_state=some-state",
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn google_calendar_callback_rejects_invalid_user_cookie() {
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let response = app
        .app
        .clone()
        .oneshot(get_with_cookies(
            "/api/v1/auth/google-calendar/callback?code=test-code&state=some-state",
            &callback_cookies("some-state", "not-a-valid-jwt"),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn google_calendar_callback_handles_provider_error() {
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
    let csrf_state = "valid-state";

    let response = app
        .app
        .clone()
        .oneshot(get_with_cookies(
            &format!("/api/v1/auth/google-calendar/callback?code=expired-code&state={csrf_state}"),
            &callback_cookies(csrf_state, &token),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 500);
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
