// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Integration tests for Garmin and Oura OAuth flows, sync jobs, and
//! disconnect endpoints.

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

fn delete_with_auth(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn get_with_auth_and_cookies(uri: &str, token: &str, cookies: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header("cookie", cookies)
        .body(Body::empty())
        .unwrap()
}

// ── Oura OAuth 2.0 flow tests ──────────────────────────────────────────

#[tokio::test]
async fn oura_login_redirects_to_auth_page() {
    let oura_mock = MockServer::start().await;
    let app = common::setup_with_config(|cfg| {
        cfg.oura_client_id = Some("test-oura-id".to_string());
        cfg.oura_client_secret = Some("test-oura-secret".to_string());
        cfg.oura_auth_base_url = Some(oura_mock.uri());
        cfg.oura_api_base_url = Some(oura_mock.uri());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(get_with_auth("/api/v1/auth/oura/login", &token))
        .await
        .unwrap();

    // Should redirect (302/303/307) to Oura's authorization page.
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
    assert!(
        location.contains("/oauth/authorize"),
        "expected Oura auth URL, got: {location}"
    );
    assert!(location.contains("client_id=test-oura-id"));

    // Should set the CSRF state cookie.
    let set_cookie = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect::<Vec<_>>()
        .join("; ");
    assert!(set_cookie.contains("oura_oauth_state="));
}

#[tokio::test]
async fn oura_login_requires_auth() {
    let app = common::setup_with_config(|cfg| {
        cfg.oura_client_id = Some("test-oura-id".to_string());
        cfg.oura_client_secret = Some("test-oura-secret".to_string());
    })
    .await;

    let response = app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/oura/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn oura_callback_exchanges_code_and_stores_tokens() {
    let oura_mock = MockServer::start().await;

    // Mock the token exchange endpoint.
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "oura-access-token-123",
            "refresh_token": "oura-refresh-token-456",
            "expires_in": 86400,
            "token_type": "bearer"
        })))
        .mount(&oura_mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.oura_client_id = Some("test-oura-id".to_string());
        cfg.oura_client_secret = Some("test-oura-secret".to_string());
        cfg.oura_api_base_url = Some(oura_mock.uri());
        cfg.oura_auth_base_url = Some(oura_mock.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;

    // Simulate the callback with a matching state cookie.
    let csrf_state = "test-csrf-state-12345";
    let response = app
        .app
        .clone()
        .oneshot(get_with_auth_and_cookies(
            &format!("/api/v1/auth/oura/callback?code=test-auth-code&state={csrf_state}"),
            &token,
            &format!("oura_oauth_state={csrf_state}"),
        ))
        .await
        .unwrap();

    // Should redirect to settings after successful token exchange.
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
    assert!(location.contains("connected=oura"));

    // Verify tokens are stored (encrypted) in the database.
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT source, access_token FROM integration_tokens WHERE user_id = $1 AND source = 'oura'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    assert_eq!(row.0, "oura");
    // Access token should be encrypted (not plaintext).
    assert_ne!(row.1, "oura-access-token-123");
    assert!(
        row.1.starts_with("v1:"),
        "encrypted token should have v1: prefix"
    );
}

#[tokio::test]
async fn oura_callback_rejects_state_mismatch() {
    let oura_mock = MockServer::start().await;

    let app = common::setup_with_config(|cfg| {
        cfg.oura_client_id = Some("test-oura-id".to_string());
        cfg.oura_client_secret = Some("test-oura-secret".to_string());
        cfg.oura_api_base_url = Some(oura_mock.uri());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(get_with_auth_and_cookies(
            "/api/v1/auth/oura/callback?code=test-code&state=wrong-state",
            &token,
            "oura_oauth_state=correct-state",
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
    let body = body_json(response).await;
    assert!(body["error"].as_str().unwrap().contains("state mismatch"));
}

#[tokio::test]
async fn oura_callback_rejects_missing_state_cookie() {
    let app = common::setup_with_config(|cfg| {
        cfg.oura_client_id = Some("test-oura-id".to_string());
        cfg.oura_client_secret = Some("test-oura-secret".to_string());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(get_with_auth(
            "/api/v1/auth/oura/callback?code=test-code&state=some-state",
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn oura_callback_handles_provider_error() {
    let oura_mock = MockServer::start().await;

    // Mock token endpoint returning an error.
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": "invalid_grant",
            "error_description": "authorization code expired"
        })))
        .mount(&oura_mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.oura_client_id = Some("test-oura-id".to_string());
        cfg.oura_client_secret = Some("test-oura-secret".to_string());
        cfg.oura_api_base_url = Some(oura_mock.uri());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;
    let csrf_state = "valid-state";

    let response = app
        .app
        .clone()
        .oneshot(get_with_auth_and_cookies(
            &format!("/api/v1/auth/oura/callback?code=expired-code&state={csrf_state}"),
            &token,
            &format!("oura_oauth_state={csrf_state}"),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 500);
}

// ── Garmin OAuth 1.0a flow tests ────────────────────────────────────────

#[tokio::test]
async fn garmin_login_redirects_to_garmin_auth() {
    let garmin_mock = MockServer::start().await;

    // Mock the request token endpoint.
    Mock::given(method("POST"))
        .and(path("/oauth-service/oauth/request_token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("oauth_token=req-token-123&oauth_token_secret=req-secret-456&oauth_callback_confirmed=true"),
        )
        .mount(&garmin_mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.garmin_client_id = Some("test-garmin-key".to_string());
        cfg.garmin_client_secret = Some("test-garmin-secret".to_string());
        cfg.garmin_base_url = Some(garmin_mock.uri());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(get_with_auth("/api/v1/auth/garmin/login", &token))
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
    assert!(location.contains("oauthConfirm"));
    assert!(location.contains("oauth_token=req-token-123"));

    // Should set cookies for the OAuth secret and token.
    let cookies: Vec<String> = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect();
    let all_cookies = cookies.join("; ");
    assert!(all_cookies.contains("garmin_oauth_secret=req-secret-456"));
    assert!(all_cookies.contains("garmin_oauth_token=req-token-123"));
}

#[tokio::test]
async fn garmin_login_requires_auth() {
    let app = common::setup_with_config(|cfg| {
        cfg.garmin_client_id = Some("test-garmin-key".to_string());
        cfg.garmin_client_secret = Some("test-garmin-secret".to_string());
    })
    .await;

    let response = app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/garmin/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn garmin_callback_exchanges_tokens_and_stores() {
    let garmin_mock = MockServer::start().await;

    // Mock the access token endpoint.
    Mock::given(method("POST"))
        .and(path("/oauth-service/oauth/access_token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(
                "oauth_token=access-token-789&oauth_token_secret=access-secret-012",
            ),
        )
        .mount(&garmin_mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.garmin_client_id = Some("test-garmin-key".to_string());
        cfg.garmin_client_secret = Some("test-garmin-secret".to_string());
        cfg.garmin_base_url = Some(garmin_mock.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(get_with_auth_and_cookies(
            "/api/v1/auth/garmin/callback?oauth_token=req-token-123&oauth_verifier=verifier-abc",
            &token,
            "garmin_oauth_secret=req-secret-456; garmin_oauth_token=req-token-123",
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
    assert!(location.contains("connected=garmin"));

    // Verify tokens are stored (encrypted).
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT source, access_token FROM integration_tokens WHERE user_id = $1 AND source = 'garmin'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    assert_eq!(row.0, "garmin");
    assert!(
        row.1.starts_with("v1:"),
        "encrypted token should have v1: prefix, got: {}",
        &row.1[..20.min(row.1.len())]
    );
}

#[tokio::test]
async fn garmin_callback_rejects_token_mismatch() {
    let garmin_mock = MockServer::start().await;

    let app = common::setup_with_config(|cfg| {
        cfg.garmin_client_id = Some("test-garmin-key".to_string());
        cfg.garmin_client_secret = Some("test-garmin-secret".to_string());
        cfg.garmin_base_url = Some(garmin_mock.uri());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(get_with_auth_and_cookies(
            "/api/v1/auth/garmin/callback?oauth_token=wrong-token&oauth_verifier=verifier",
            &token,
            "garmin_oauth_secret=some-secret; garmin_oauth_token=original-token",
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
    let body = body_json(response).await;
    assert!(body["error"].as_str().unwrap().contains("mismatch"));
}

// ── Integration list and disconnect tests ───────────────────────────────

#[tokio::test]
async fn list_integrations_shows_connected_sources() {
    let oura_mock = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "test-access",
            "refresh_token": "test-refresh",
            "expires_in": 86400,
            "token_type": "bearer"
        })))
        .mount(&oura_mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.oura_client_id = Some("test-oura-id".to_string());
        cfg.oura_client_secret = Some("test-oura-secret".to_string());
        cfg.oura_api_base_url = Some(oura_mock.uri());
    })
    .await;

    let (_user_id, token) = common::create_test_user(&app).await;

    // Connect Oura via the callback.
    let csrf_state = "csrf-for-list-test";
    app.app
        .clone()
        .oneshot(get_with_auth_and_cookies(
            &format!("/api/v1/auth/oura/callback?code=test-code&state={csrf_state}"),
            &token,
            &format!("oura_oauth_state={csrf_state}"),
        ))
        .await
        .unwrap();

    // List integrations.
    let response = app
        .app
        .clone()
        .oneshot(get_with_auth("/api/v1/integrations", &token))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = body_json(response).await;
    let integrations = body.as_array().unwrap();
    assert!(
        integrations.iter().any(|i| i["source"] == "oura"),
        "expected oura in integrations list: {body:?}"
    );
}

#[tokio::test]
async fn disconnect_integration_removes_tokens() {
    let oura_mock = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "test-access",
            "refresh_token": "test-refresh",
            "expires_in": 86400,
            "token_type": "bearer"
        })))
        .mount(&oura_mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.oura_client_id = Some("test-oura-id".to_string());
        cfg.oura_client_secret = Some("test-oura-secret".to_string());
        cfg.oura_api_base_url = Some(oura_mock.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;

    // Connect Oura first.
    let csrf_state = "csrf-for-disconnect";
    app.app
        .clone()
        .oneshot(get_with_auth_and_cookies(
            &format!("/api/v1/auth/oura/callback?code=test-code&state={csrf_state}"),
            &token,
            &format!("oura_oauth_state={csrf_state}"),
        ))
        .await
        .unwrap();

    // Verify it's connected.
    let count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM integration_tokens WHERE user_id = $1 AND source = 'oura'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(count.0, 1);

    // Disconnect.
    let response = app
        .app
        .clone()
        .oneshot(delete_with_auth("/api/v1/integrations/oura", &token))
        .await
        .unwrap();

    assert_eq!(response.status(), 204);

    // Verify tokens are deleted.
    let count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM integration_tokens WHERE user_id = $1 AND source = 'oura'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(count.0, 0);
}

#[tokio::test]
async fn disconnect_nonexistent_returns_204() {
    let app = common::setup().await;
    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(delete_with_auth("/api/v1/integrations/garmin", &token))
        .await
        .unwrap();

    // Deleting a non-connected integration should succeed silently.
    assert_eq!(response.status(), 204);
}

// ── Oura client unit-style tests (WireMock) ─────────────────────────────

#[tokio::test]
async fn oura_client_exchanges_code_for_tokens() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "new-access",
            "refresh_token": "new-refresh",
            "expires_in": 3600,
            "token_type": "bearer"
        })))
        .mount(&mock_server)
        .await;

    let client = api::integrations::oura::OuraClient::new(
        "client-id".to_string(),
        "client-secret".to_string(),
        Some(mock_server.uri()),
        None,
        reqwest::Client::new(),
    );

    let result = client
        .exchange_code("auth-code-123", "https://example.com/callback")
        .await;

    let tokens = result.unwrap();
    assert_eq!(tokens.access_token, "new-access");
    assert_eq!(tokens.refresh_token.unwrap(), "new-refresh");
    assert_eq!(tokens.expires_in.unwrap(), 3600);
}

#[tokio::test]
async fn oura_client_refreshes_token() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "refreshed-access",
            "refresh_token": "refreshed-refresh",
            "expires_in": 7200,
            "token_type": "bearer"
        })))
        .mount(&mock_server)
        .await;

    let client = api::integrations::oura::OuraClient::new(
        "client-id".to_string(),
        "client-secret".to_string(),
        Some(mock_server.uri()),
        None,
        reqwest::Client::new(),
    );

    let result = client.refresh_token("old-refresh-token").await;

    let tokens = result.unwrap();
    assert_eq!(tokens.access_token, "refreshed-access");
}

#[tokio::test]
async fn oura_client_handles_api_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": "invalid_client"
        })))
        .mount(&mock_server)
        .await;

    let client = api::integrations::oura::OuraClient::new(
        "bad-id".to_string(),
        "bad-secret".to_string(),
        Some(mock_server.uri()),
        None,
        reqwest::Client::new(),
    );

    let result = client
        .exchange_code("code", "https://example.com/callback")
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("401"));
}

#[tokio::test]
async fn oura_client_fetches_daily_readiness() {
    let mock_server = MockServer::start().await;

    let fixture = include_str!("../fixtures/oura/daily-readiness.json");
    Mock::given(method("GET"))
        .and(path("/v2/usercollection/daily_readiness"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture))
        .mount(&mock_server)
        .await;

    let client = api::integrations::oura::OuraClient::new(
        "id".to_string(),
        "secret".to_string(),
        Some(mock_server.uri()),
        None,
        reqwest::Client::new(),
    );

    let result = client
        .get_daily_readiness("access-token", "2026-03-28", "2026-03-28")
        .await;

    let response = result.unwrap();
    assert_eq!(response.data.len(), 1);
    assert_eq!(response.data[0].score, Some(85.0));
    assert_eq!(
        response.data[0].contributors.as_ref().unwrap().hrv_balance,
        Some(45.0)
    );
}

#[tokio::test]
async fn oura_client_fetches_daily_sleep() {
    let mock_server = MockServer::start().await;

    let fixture = include_str!("../fixtures/oura/daily-sleep.json");
    Mock::given(method("GET"))
        .and(path("/v2/usercollection/daily_sleep"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture))
        .mount(&mock_server)
        .await;

    let client = api::integrations::oura::OuraClient::new(
        "id".to_string(),
        "secret".to_string(),
        Some(mock_server.uri()),
        None,
        reqwest::Client::new(),
    );

    let result = client
        .get_daily_sleep("access-token", "2026-03-28", "2026-03-28")
        .await;

    let response = result.unwrap();
    assert_eq!(response.data.len(), 1);
    assert_eq!(response.data[0].score, Some(88.0));
    assert_eq!(response.data[0].deep_sleep_duration, Some(4800));
}

#[tokio::test]
async fn oura_client_fetches_daily_activity() {
    let mock_server = MockServer::start().await;

    let fixture = include_str!("../fixtures/oura/daily-activity.json");
    Mock::given(method("GET"))
        .and(path("/v2/usercollection/daily_activity"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture))
        .mount(&mock_server)
        .await;

    let client = api::integrations::oura::OuraClient::new(
        "id".to_string(),
        "secret".to_string(),
        Some(mock_server.uri()),
        None,
        reqwest::Client::new(),
    );

    let result = client
        .get_daily_activity("access-token", "2026-03-28", "2026-03-28")
        .await;

    let response = result.unwrap();
    assert_eq!(response.data.len(), 1);
    assert_eq!(response.data[0].steps, Some(9200));
}

// ── Garmin client WireMock tests ────────────────────────────────────────

#[tokio::test]
async fn garmin_client_fetches_daily_summary() {
    let mock_server = MockServer::start().await;

    let fixture = include_str!("../fixtures/garmin/daily-summary.json");
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/dailies"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture))
        .mount(&mock_server)
        .await;

    let client = api::integrations::garmin::GarminClient::new(
        "key".to_string(),
        "secret".to_string(),
        Some(mock_server.uri()),
        reqwest::Client::new(),
    );

    let token = api::integrations::garmin::AccessToken {
        oauth_token: "token".to_string(),
        oauth_token_secret: "secret".to_string(),
    };

    let result = client
        .get_daily_summary(&token, "2026-03-27", "2026-03-28")
        .await;

    let summaries = result.unwrap();
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].total_steps, Some(8543));
    assert_eq!(summaries[1].resting_heart_rate, Some(58.0));
}

#[tokio::test]
async fn garmin_client_fetches_sleep() {
    let mock_server = MockServer::start().await;

    let fixture = include_str!("../fixtures/garmin/sleep.json");
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/sleeps"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture))
        .mount(&mock_server)
        .await;

    let client = api::integrations::garmin::GarminClient::new(
        "key".to_string(),
        "secret".to_string(),
        Some(mock_server.uri()),
        reqwest::Client::new(),
    );

    let token = api::integrations::garmin::AccessToken {
        oauth_token: "token".to_string(),
        oauth_token_secret: "secret".to_string(),
    };

    let result = client.get_sleep(&token, "2026-03-28", "2026-03-28").await;

    let sleeps = result.unwrap();
    assert_eq!(sleeps.len(), 1);
    assert_eq!(sleeps[0].deep_sleep_seconds, Some(4800));
    assert_eq!(sleeps[0].overall_score, Some(82.0));
}

#[tokio::test]
async fn garmin_client_fetches_hrv() {
    let mock_server = MockServer::start().await;

    let fixture = include_str!("../fixtures/garmin/hrv.json");
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/hrv"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture))
        .mount(&mock_server)
        .await;

    let client = api::integrations::garmin::GarminClient::new(
        "key".to_string(),
        "secret".to_string(),
        Some(mock_server.uri()),
        reqwest::Client::new(),
    );

    let token = api::integrations::garmin::AccessToken {
        oauth_token: "token".to_string(),
        oauth_token_secret: "secret".to_string(),
    };

    let result = client.get_hrv(&token, "2026-03-28", "2026-03-28").await;

    let hrvs = result.unwrap();
    assert_eq!(hrvs.len(), 1);
    assert_eq!(hrvs[0].last_night, Some(45.0));
    assert_eq!(hrvs[0].status, Some("BALANCED".to_string()));
}

#[tokio::test]
async fn garmin_client_fetches_body_comp() {
    let mock_server = MockServer::start().await;

    let fixture = include_str!("../fixtures/garmin/body-comp.json");
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/bodyComps"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture))
        .mount(&mock_server)
        .await;

    let client = api::integrations::garmin::GarminClient::new(
        "key".to_string(),
        "secret".to_string(),
        Some(mock_server.uri()),
        reqwest::Client::new(),
    );

    let token = api::integrations::garmin::AccessToken {
        oauth_token: "token".to_string(),
        oauth_token_secret: "secret".to_string(),
    };

    let result = client
        .get_body_comp(&token, "2026-03-28", "2026-03-28")
        .await;

    let body_comps = result.unwrap();
    assert_eq!(body_comps.len(), 1);
    assert_eq!(body_comps[0].weight, Some(75200.0));
    assert_eq!(body_comps[0].body_fat, Some(18.2));
}

#[tokio::test]
async fn garmin_client_handles_api_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/dailies"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
        .mount(&mock_server)
        .await;

    let client = api::integrations::garmin::GarminClient::new(
        "key".to_string(),
        "secret".to_string(),
        Some(mock_server.uri()),
        reqwest::Client::new(),
    );

    let token = api::integrations::garmin::AccessToken {
        oauth_token: "token".to_string(),
        oauth_token_secret: "secret".to_string(),
    };

    let result: Result<Vec<api::integrations::garmin::GarminDailySummary>, String> = client
        .get_daily_summary(&token, "2026-03-28", "2026-03-28")
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("403"));
}

#[tokio::test]
async fn garmin_client_handles_malformed_response() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/dailies"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json at all"))
        .mount(&mock_server)
        .await;

    let client = api::integrations::garmin::GarminClient::new(
        "key".to_string(),
        "secret".to_string(),
        Some(mock_server.uri()),
        reqwest::Client::new(),
    );

    let token = api::integrations::garmin::AccessToken {
        oauth_token: "token".to_string(),
        oauth_token_secret: "secret".to_string(),
    };

    let result: Result<Vec<api::integrations::garmin::GarminDailySummary>, String> = client
        .get_daily_summary(&token, "2026-03-28", "2026-03-28")
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("parse"));
}

// ── DB integration_tokens tests ─────────────────────────────────────────

#[tokio::test]
async fn integration_tokens_upsert_encrypts_and_round_trips() {
    let app = common::setup().await;
    let (user_id, _) = common::create_test_user(&app).await;

    let key = api::crypto::parse_encryption_key(
        "0000000000000000000000000000000000000000000000000000000000000000",
    )
    .unwrap();

    let row = api::db::integration_tokens::upsert(
        &app.pool,
        user_id,
        "garmin",
        "my-access-token",
        Some("my-secret"),
        None,
        &key,
    )
    .await
    .unwrap();

    // The returned row has decrypted values.
    assert_eq!(row.access_token, "my-access-token");
    assert_eq!(row.refresh_token.as_deref(), Some("my-secret"));

    // But the database has encrypted values.
    let db_row =
        sqlx::query_as::<_, (String,)>("SELECT access_token FROM integration_tokens WHERE id = $1")
            .bind(row.id)
            .fetch_one(&app.pool)
            .await
            .unwrap();

    assert_ne!(db_row.0, "my-access-token");
    assert!(db_row.0.starts_with("v1:"));
}

#[tokio::test]
async fn integration_tokens_upsert_updates_on_conflict() {
    let app = common::setup().await;
    let (user_id, _) = common::create_test_user(&app).await;

    let key = api::crypto::parse_encryption_key(
        "0000000000000000000000000000000000000000000000000000000000000000",
    )
    .unwrap();

    // First insert.
    api::db::integration_tokens::upsert(
        &app.pool,
        user_id,
        "oura",
        "token-v1",
        Some("refresh-v1"),
        None,
        &key,
    )
    .await
    .unwrap();

    // Upsert with new values.
    let row = api::db::integration_tokens::upsert(
        &app.pool,
        user_id,
        "oura",
        "token-v2",
        Some("refresh-v2"),
        None,
        &key,
    )
    .await
    .unwrap();

    assert_eq!(row.access_token, "token-v2");
    assert_eq!(row.refresh_token.as_deref(), Some("refresh-v2"));

    // Only one row should exist.
    let count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM integration_tokens WHERE user_id = $1 AND source = 'oura'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(count.0, 1);
}

#[tokio::test]
async fn integration_tokens_list_by_source() {
    let app = common::setup().await;
    let (user_id, _) = common::create_test_user(&app).await;

    let key = api::crypto::parse_encryption_key(
        "0000000000000000000000000000000000000000000000000000000000000000",
    )
    .unwrap();

    api::db::integration_tokens::upsert(
        &app.pool,
        user_id,
        "garmin",
        "garmin-token",
        Some("garmin-secret"),
        None,
        &key,
    )
    .await
    .unwrap();

    api::db::integration_tokens::upsert(
        &app.pool,
        user_id,
        "oura",
        "oura-token",
        Some("oura-refresh"),
        None,
        &key,
    )
    .await
    .unwrap();

    let garmin_tokens =
        api::db::integration_tokens::list_for_user_by_source(&app.pool, "garmin", &key, None)
            .await
            .unwrap();
    assert_eq!(garmin_tokens.len(), 1);
    assert_eq!(garmin_tokens[0].access_token, "garmin-token");

    let oura_tokens =
        api::db::integration_tokens::list_for_user_by_source(&app.pool, "oura", &key, None)
            .await
            .unwrap();
    assert_eq!(oura_tokens.len(), 1);
    assert_eq!(oura_tokens[0].access_token, "oura-token");
}

#[tokio::test]
async fn integration_tokens_update_sync_status() {
    let app = common::setup().await;
    let (user_id, _) = common::create_test_user(&app).await;

    let key = api::crypto::parse_encryption_key(
        "0000000000000000000000000000000000000000000000000000000000000000",
    )
    .unwrap();

    api::db::integration_tokens::upsert(&app.pool, user_id, "garmin", "token", None, None, &key)
        .await
        .unwrap();

    // Update last_synced_at.
    api::db::integration_tokens::update_last_synced(&app.pool, user_id, "garmin")
        .await
        .unwrap();

    let row = sqlx::query_as::<_, (Option<chrono::DateTime<chrono::Utc>>, Option<String>)>(
        "SELECT last_synced_at, last_sync_error FROM integration_tokens WHERE user_id = $1 AND source = 'garmin'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    assert!(row.0.is_some());
    assert!(row.1.is_none());

    // Update sync error.
    api::db::integration_tokens::update_sync_error(
        &app.pool,
        user_id,
        "garmin",
        "connection timeout",
    )
    .await
    .unwrap();

    let row = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT last_sync_error FROM integration_tokens WHERE user_id = $1 AND source = 'garmin'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    assert_eq!(row.0.as_deref(), Some("connection timeout"));
}
