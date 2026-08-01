// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Integration tests for the Garmin/Oura manual sync endpoints and the
//! per-user sync bookkeeping: a fully successful sync inserts records and
//! advances `last_synced_at`; a partial fetch failure leaves the watermark
//! untouched and records `last_sync_error` instead of silently losing the
//! failed window; and a subsequent success clears the error.

use axum::body::Body;
use chrono::{DateTime, Utc};
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

fn post_with_auth(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

fn test_encryption_key() -> [u8; 32] {
    api::crypto::parse_encryption_key(
        "0000000000000000000000000000000000000000000000000000000000000000",
    )
    .unwrap()
}

async fn sync_status(
    pool: &sqlx::PgPool,
    user_id: uuid::Uuid,
    source: &str,
) -> (Option<DateTime<Utc>>, Option<String>) {
    sqlx::query_as::<_, (Option<DateTime<Utc>>, Option<String>)>(
        "SELECT last_synced_at, last_sync_error FROM integration_tokens \
         WHERE user_id = $1 AND source = $2",
    )
    .bind(user_id)
    .bind(source)
    .fetch_one(pool)
    .await
    .unwrap()
}

// ── Garmin manual sync ──────────────────────────────────────────────────

async fn mount_garmin(garmin_mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/dailies"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/garmin/daily-summary.json")),
        )
        .mount(garmin_mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/sleeps"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/garmin/sleep.json")),
        )
        .mount(garmin_mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/hrv"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(include_str!("../fixtures/garmin/hrv.json")),
        )
        .mount(garmin_mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/bodyComps"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/garmin/body-comp.json")),
        )
        .mount(garmin_mock)
        .await;
}

#[tokio::test]
async fn garmin_manual_sync_requires_auth() {
    let app = common::setup().await;

    let response = app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/integrations/garmin/sync")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn garmin_manual_sync_without_connection_returns_404() {
    let app = common::setup_with_config(|cfg| {
        cfg.garmin_client_id = Some("test-garmin-key".to_string());
        cfg.garmin_client_secret = Some("test-garmin-secret".to_string());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(post_with_auth("/api/v1/integrations/garmin/sync", &token))
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn garmin_manual_sync_inserts_records_and_advances_watermark() {
    let garmin_mock = MockServer::start().await;
    mount_garmin(&garmin_mock).await;

    let app = common::setup_with_config(|cfg| {
        cfg.garmin_client_id = Some("test-garmin-key".to_string());
        cfg.garmin_client_secret = Some("test-garmin-secret".to_string());
        cfg.garmin_base_url = Some(garmin_mock.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;
    let key = test_encryption_key();
    api::db::integration_tokens::upsert(
        &app.pool,
        user_id,
        "garmin",
        "garmin-access",
        Some("garmin-secret"),
        None,
        &key,
    )
    .await
    .unwrap();

    let response = app
        .app
        .clone()
        .oneshot(post_with_auth("/api/v1/integrations/garmin/sync", &token))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = body_json(response).await;
    assert_eq!(body["source"], "garmin");
    assert!(
        body["records_inserted"].as_u64().unwrap() > 0,
        "expected records_inserted > 0, got {body:?}"
    );

    let count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM health_records WHERE user_id = $1 AND source = 'garmin'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert!(count.0 > 0);

    let (last_synced_at, last_sync_error) = sync_status(&app.pool, user_id, "garmin").await;
    assert!(
        last_synced_at.is_some(),
        "watermark should advance on full success"
    );
    assert!(last_sync_error.is_none());
}

#[tokio::test]
async fn garmin_manual_sync_partial_failure_leaves_watermark_then_clears_on_success() {
    let garmin_mock = MockServer::start().await;

    // Three fetches succeed; sleeps fails once, then succeeds on retry.
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/dailies"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/garmin/daily-summary.json")),
        )
        .mount(&garmin_mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/hrv"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(include_str!("../fixtures/garmin/hrv.json")),
        )
        .mount(&garmin_mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/bodyComps"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/garmin/body-comp.json")),
        )
        .mount(&garmin_mock)
        .await;
    // First call to /sleeps fails; subsequent calls succeed.
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/sleeps"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&garmin_mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/sleeps"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/garmin/sleep.json")),
        )
        .with_priority(2)
        .mount(&garmin_mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.garmin_client_id = Some("test-garmin-key".to_string());
        cfg.garmin_client_secret = Some("test-garmin-secret".to_string());
        cfg.garmin_base_url = Some(garmin_mock.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;
    let key = test_encryption_key();
    api::db::integration_tokens::upsert(
        &app.pool,
        user_id,
        "garmin",
        "garmin-access",
        Some("garmin-secret"),
        None,
        &key,
    )
    .await
    .unwrap();

    // Establish a baseline watermark before the failing sync so we can prove
    // it doesn't move when the sync partially fails.
    api::db::integration_tokens::update_last_synced(&app.pool, user_id, "garmin")
        .await
        .unwrap();
    let (baseline_synced_at, _) = sync_status(&app.pool, user_id, "garmin").await;

    // First sync: sleeps fetch fails -> the whole sync is reported as failed.
    let first = app
        .app
        .clone()
        .oneshot(post_with_auth("/api/v1/integrations/garmin/sync", &token))
        .await
        .unwrap();
    assert_eq!(first.status(), 500);

    let (after_failure_synced_at, after_failure_error) =
        sync_status(&app.pool, user_id, "garmin").await;
    assert_eq!(
        after_failure_synced_at, baseline_synced_at,
        "watermark must not advance when a fetch fails"
    );
    assert!(
        after_failure_error.is_some(),
        "last_sync_error must be recorded on partial failure"
    );

    // Second sync: every fetch succeeds now -> watermark advances, error clears.
    let second = app
        .app
        .clone()
        .oneshot(post_with_auth("/api/v1/integrations/garmin/sync", &token))
        .await
        .unwrap();
    assert_eq!(second.status(), 200);

    let (after_success_synced_at, after_success_error) =
        sync_status(&app.pool, user_id, "garmin").await;
    assert!(
        after_success_synced_at.unwrap() > baseline_synced_at.unwrap(),
        "watermark should advance once the sync fully succeeds"
    );
    assert!(
        after_success_error.is_none(),
        "a subsequent success must clear last_sync_error"
    );
}

// ── Oura manual sync ─────────────────────────────────────────────────────

async fn mount_oura(oura_mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/v2/usercollection/daily_readiness"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/oura/daily-readiness.json")),
        )
        .mount(oura_mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/usercollection/daily_sleep"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/oura/daily-sleep.json")),
        )
        .mount(oura_mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/usercollection/daily_activity"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/oura/daily-activity.json")),
        )
        .mount(oura_mock)
        .await;
}

#[tokio::test]
async fn oura_manual_sync_requires_auth() {
    let app = common::setup().await;

    let response = app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/integrations/oura/sync")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn oura_manual_sync_without_connection_returns_404() {
    let app = common::setup_with_config(|cfg| {
        cfg.oura_client_id = Some("test-oura-id".to_string());
        cfg.oura_client_secret = Some("test-oura-secret".to_string());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(post_with_auth("/api/v1/integrations/oura/sync", &token))
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn oura_manual_sync_inserts_records_and_advances_watermark() {
    let oura_mock = MockServer::start().await;
    mount_oura(&oura_mock).await;

    let app = common::setup_with_config(|cfg| {
        cfg.oura_client_id = Some("test-oura-id".to_string());
        cfg.oura_client_secret = Some("test-oura-secret".to_string());
        cfg.oura_api_base_url = Some(oura_mock.uri());
        cfg.oura_auth_base_url = Some(oura_mock.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;
    let key = test_encryption_key();
    api::db::integration_tokens::upsert(
        &app.pool,
        user_id,
        "oura",
        "oura-access",
        Some("oura-refresh"),
        None,
        &key,
    )
    .await
    .unwrap();

    let response = app
        .app
        .clone()
        .oneshot(post_with_auth("/api/v1/integrations/oura/sync", &token))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = body_json(response).await;
    assert_eq!(body["source"], "oura");
    assert!(
        body["records_inserted"].as_u64().unwrap() > 0,
        "expected records_inserted > 0, got {body:?}"
    );

    let count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM health_records WHERE user_id = $1 AND source = 'oura'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert!(count.0 > 0);

    let (last_synced_at, last_sync_error) = sync_status(&app.pool, user_id, "oura").await;
    assert!(
        last_synced_at.is_some(),
        "watermark should advance on full success"
    );
    assert!(last_sync_error.is_none());
}

#[tokio::test]
async fn oura_manual_sync_partial_failure_leaves_watermark_then_clears_on_success() {
    let oura_mock = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v2/usercollection/daily_readiness"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/oura/daily-readiness.json")),
        )
        .mount(&oura_mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/usercollection/daily_activity"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/oura/daily-activity.json")),
        )
        .mount(&oura_mock)
        .await;
    // First call to daily_sleep fails; subsequent calls succeed.
    Mock::given(method("GET"))
        .and(path("/v2/usercollection/daily_sleep"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&oura_mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/v2/usercollection/daily_sleep"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/oura/daily-sleep.json")),
        )
        .with_priority(2)
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
    let key = test_encryption_key();
    api::db::integration_tokens::upsert(
        &app.pool,
        user_id,
        "oura",
        "oura-access",
        Some("oura-refresh"),
        None,
        &key,
    )
    .await
    .unwrap();

    api::db::integration_tokens::update_last_synced(&app.pool, user_id, "oura")
        .await
        .unwrap();
    let (baseline_synced_at, _) = sync_status(&app.pool, user_id, "oura").await;

    let first = app
        .app
        .clone()
        .oneshot(post_with_auth("/api/v1/integrations/oura/sync", &token))
        .await
        .unwrap();
    assert_eq!(first.status(), 500);

    let (after_failure_synced_at, after_failure_error) =
        sync_status(&app.pool, user_id, "oura").await;
    assert_eq!(
        after_failure_synced_at, baseline_synced_at,
        "watermark must not advance when a fetch fails"
    );
    assert!(
        after_failure_error.is_some(),
        "last_sync_error must be recorded on partial failure"
    );

    let second = app
        .app
        .clone()
        .oneshot(post_with_auth("/api/v1/integrations/oura/sync", &token))
        .await
        .unwrap();
    assert_eq!(second.status(), 200);

    let (after_success_synced_at, after_success_error) =
        sync_status(&app.pool, user_id, "oura").await;
    assert!(
        after_success_synced_at.unwrap() > baseline_synced_at.unwrap(),
        "watermark should advance once the sync fully succeeds"
    );
    assert!(
        after_success_error.is_none(),
        "a subsequent success must clear last_sync_error"
    );
}
