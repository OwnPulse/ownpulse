// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Integration tests for the MyChart / SMART-on-FHIR connect + sync flow and
//! FHIR Observation parsing into `lab_results`.

use axum::body::Body;
use http::Request;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common;

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn post_with_auth(uri: &str, token: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .unwrap()
}

// ── connect ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn mychart_connect_exchanges_code_and_stores_encrypted_tokens() {
    let fhir_mock = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth2/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "mychart-access-123",
            "refresh_token": "mychart-refresh-456",
            "expires_in": 3600,
            "token_type": "bearer"
        })))
        .mount(&fhir_mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.mychart_client_id = Some("test-mychart-client".to_string());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;

    let req_body = serde_json::json!({
        "fhir_base_url": format!("{}/fhir/r4", fhir_mock.uri()),
        "token_endpoint": format!("{}/oauth2/token", fhir_mock.uri()),
        "code": "auth-code-abc",
        "redirect_uri": "ownpulse://mychart-callback",
        "code_verifier": "pkce-verifier-xyz"
    });

    let response = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/mychart/connect",
            &token,
            &req_body,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = body_json(response).await;
    assert_eq!(body["source"], "mychart");
    assert_eq!(body["connected"], true);

    // Token stored encrypted, and FHIR metadata persisted.
    let row = sqlx::query_as::<_, (String, Option<Value>)>(
        "SELECT access_token, metadata FROM integration_tokens WHERE user_id = $1 AND source = 'mychart'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    assert_ne!(row.0, "mychart-access-123");
    assert!(row.0.starts_with("v1:"), "token should be encrypted");
    let metadata = row.1.expect("metadata should be stored");
    assert_eq!(
        metadata["token_endpoint"],
        format!("{}/oauth2/token", fhir_mock.uri())
    );
}

#[tokio::test]
async fn mychart_connect_requires_auth() {
    let app = common::setup_with_config(|cfg| {
        cfg.mychart_client_id = Some("test-mychart-client".to_string());
    })
    .await;

    let req_body = serde_json::json!({
        "fhir_base_url": "https://fhir.example.org/r4",
        "token_endpoint": "https://fhir.example.org/oauth2/token",
        "code": "code",
        "redirect_uri": "ownpulse://cb",
        "code_verifier": "v"
    });

    let response = app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/integrations/mychart/connect")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&req_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn mychart_connect_rejects_internal_url_when_validation_enabled() {
    // With the SSRF guard active (insecure URLs disallowed), a request that
    // points the server at an internal/non-HTTPS host is rejected before any
    // outbound request is made.
    let app = common::setup_with_config(|cfg| {
        cfg.mychart_client_id = Some("test-mychart-client".to_string());
        cfg.mychart_allow_insecure_urls = false;
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let req_body = serde_json::json!({
        "fhir_base_url": "http://169.254.169.254/latest/meta-data",
        "token_endpoint": "http://169.254.169.254/oauth2/token",
        "code": "code",
        "redirect_uri": "ownpulse://cb",
        "code_verifier": "v"
    });

    let response = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/mychart/connect",
            &token,
            &req_body,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn mychart_connect_handles_token_endpoint_error() {
    let fhir_mock = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth2/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": "invalid_grant"
        })))
        .mount(&fhir_mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.mychart_client_id = Some("test-mychart-client".to_string());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let req_body = serde_json::json!({
        "fhir_base_url": format!("{}/fhir/r4", fhir_mock.uri()),
        "token_endpoint": format!("{}/oauth2/token", fhir_mock.uri()),
        "code": "expired",
        "redirect_uri": "ownpulse://cb",
        "code_verifier": "v"
    });

    let response = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/mychart/connect",
            &token,
            &req_body,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 500);
}

// ── sync ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn mychart_sync_imports_lab_observations() {
    let fhir_mock = MockServer::start().await;

    // Token exchange for connect.
    Mock::given(method("POST"))
        .and(path("/oauth2/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "mychart-access-123",
            "refresh_token": "mychart-refresh-456",
            "expires_in": 3600,
            "token_type": "bearer"
        })))
        .mount(&fhir_mock)
        .await;

    // FHIR Observation search.
    let fixture = include_str!("../fixtures/mychart/observation-bundle.json");
    Mock::given(method("GET"))
        .and(path("/fhir/r4/Observation"))
        .and(query_param("category", "laboratory"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture))
        .mount(&fhir_mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.mychart_client_id = Some("test-mychart-client".to_string());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;

    // Connect first.
    let connect_body = serde_json::json!({
        "fhir_base_url": format!("{}/fhir/r4", fhir_mock.uri()),
        "token_endpoint": format!("{}/oauth2/token", fhir_mock.uri()),
        "code": "auth-code-abc",
        "redirect_uri": "ownpulse://mychart-callback",
        "code_verifier": "pkce-verifier-xyz"
    });
    let connect = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/mychart/connect",
            &token,
            &connect_body,
        ))
        .await
        .unwrap();
    assert_eq!(connect.status(), 200);

    // Sync.
    let response = app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/integrations/mychart/sync")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = body_json(response).await;
    // Two of the three observations are numeric labs; the narrative-only one is skipped.
    assert_eq!(body["imported"], 2);
    assert_eq!(body["source"], "mychart");

    // Rows landed in lab_results with source = mychart and FHIR ids.
    let rows = sqlx::query_as::<_, (String, f64, String, Option<String>)>(
        "SELECT marker, value, source, source_id FROM lab_results WHERE user_id = $1 ORDER BY marker",
    )
    .bind(user_id)
    .fetch_all(&app.pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.2 == "mychart"));
    let glucose = rows.iter().find(|r| r.0 == "Glucose").unwrap();
    assert_eq!(glucose.1, 92.0);
    assert_eq!(glucose.3.as_deref(), Some("obs-glucose-1"));
}

#[tokio::test]
async fn mychart_sync_is_idempotent() {
    let fhir_mock = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth2/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "mychart-access-123",
            "refresh_token": "mychart-refresh-456",
            "expires_in": 3600,
            "token_type": "bearer"
        })))
        .mount(&fhir_mock)
        .await;

    let fixture = include_str!("../fixtures/mychart/observation-bundle.json");
    Mock::given(method("GET"))
        .and(path("/fhir/r4/Observation"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture))
        .mount(&fhir_mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.mychart_client_id = Some("test-mychart-client".to_string());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;

    let connect_body = serde_json::json!({
        "fhir_base_url": format!("{}/fhir/r4", fhir_mock.uri()),
        "token_endpoint": format!("{}/oauth2/token", fhir_mock.uri()),
        "code": "auth-code-abc",
        "redirect_uri": "ownpulse://mychart-callback",
        "code_verifier": "pkce-verifier-xyz"
    });
    app.app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/mychart/connect",
            &token,
            &connect_body,
        ))
        .await
        .unwrap();

    let sync = |app: axum::Router, token: String| async move {
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/integrations/mychart/sync")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    };

    let first = sync(app.app.clone(), token.clone()).await;
    assert_eq!(body_json(first).await["imported"], 2);

    // Second sync imports nothing new (dedup by source_id).
    let second = sync(app.app.clone(), token.clone()).await;
    let body = body_json(second).await;
    assert_eq!(body["imported"], 0);
    assert_eq!(body["skipped"], 2);

    let count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM lab_results WHERE user_id = $1 AND source = 'mychart'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(count.0, 2);
}

#[tokio::test]
async fn mychart_sync_without_connection_fails() {
    let app = common::setup_with_config(|cfg| {
        cfg.mychart_client_id = Some("test-mychart-client".to_string());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/integrations/mychart/sync")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Not connected -> the sync layer returns an error surfaced as 500.
    assert_eq!(response.status(), 500);
}

// ── FHIR client + parser (WireMock) ──────────────────────────────────────

#[tokio::test]
async fn mychart_client_fetches_and_parses_observation_bundle() {
    let mock_server = MockServer::start().await;

    let fixture = include_str!("../fixtures/mychart/observation-bundle.json");
    Mock::given(method("GET"))
        .and(path("/Observation"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture))
        .mount(&mock_server)
        .await;

    let client = api::integrations::mychart::MyChartClient::new(
        "client-id".to_string(),
        format!("{}/oauth2/token", mock_server.uri()),
        mock_server.uri(),
        reqwest::Client::new(),
    );

    let bundle = client.get_lab_observations("access-token").await.unwrap();
    let labs = api::integrations::mychart::parse_observation_bundle(&bundle);

    assert_eq!(labs.len(), 2);
    assert!(
        labs.iter()
            .any(|l| l.marker == "Glucose" && l.value == 92.0)
    );
    assert!(labs.iter().any(|l| l.marker == "Hemoglobin A1c"));
}

#[tokio::test]
async fn mychart_diagnostic_report_bundle_yields_no_lab_rows() {
    // A DiagnosticReport bundle references Observations but carries no lab
    // values of its own; our importer pulls values from Observations only.
    let fixture = include_str!("../fixtures/mychart/diagnostic-report.json");
    let bundle: api::integrations::mychart::FhirBundle = serde_json::from_str(fixture).unwrap();
    let labs = api::integrations::mychart::parse_observation_bundle(&bundle);
    assert!(labs.is_empty());
}

#[tokio::test]
async fn mychart_client_exchanges_code() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth2/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "new-access",
            "refresh_token": "new-refresh",
            "expires_in": 3600
        })))
        .mount(&mock_server)
        .await;

    let client = api::integrations::mychart::MyChartClient::new(
        "client-id".to_string(),
        format!("{}/oauth2/token", mock_server.uri()),
        mock_server.uri(),
        reqwest::Client::new(),
    );

    let tokens = client
        .exchange_code("code-123", "ownpulse://cb", "verifier")
        .await
        .unwrap();
    assert_eq!(tokens.access_token, "new-access");
    assert_eq!(tokens.refresh_token.as_deref(), Some("new-refresh"));
}

#[tokio::test]
async fn mychart_client_handles_observation_fetch_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/Observation"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
        .mount(&mock_server)
        .await;

    let client = api::integrations::mychart::MyChartClient::new(
        "client-id".to_string(),
        format!("{}/oauth2/token", mock_server.uri()),
        mock_server.uri(),
        reqwest::Client::new(),
    );

    let result = client.get_lab_observations("access-token").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("403"));
}
