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
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::common;

/// Row shape used for the record-level assertions below.
type HealthRecordRow = (String, Option<f64>, Option<String>);

async fn health_record_by_type(
    pool: &sqlx::PgPool,
    user_id: uuid::Uuid,
    record_type: &str,
) -> HealthRecordRow {
    sqlx::query_as::<_, HealthRecordRow>(
        "SELECT record_type, value, unit FROM health_records \
         WHERE user_id = $1 AND source = 'garmin' AND record_type = $2",
    )
    .bind(user_id)
    .bind(record_type)
    .fetch_one(pool)
    .await
    .unwrap()
}

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

/// Push `integration_tokens.updated_at` back in time so a subsequent manual
/// sync call in a test isn't rejected by the per-user cooldown (see
/// `MIN_SYNC_INTERVAL_SECS` in `jobs::garmin_sync`/`jobs::oura_sync`) — tests
/// otherwise run several sync attempts back-to-back with no real time
/// elapsed between them.
async fn backdate_last_attempt(pool: &sqlx::PgPool, user_id: uuid::Uuid, source: &str) {
    sqlx::query(
        "UPDATE integration_tokens SET updated_at = now() - interval '2 minutes' \
         WHERE user_id = $1 AND source = $2",
    )
    .bind(user_id)
    .bind(source)
    .execute(pool)
    .await
    .unwrap();
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
    // it doesn't move when the sync partially fails. Backdated so the first
    // manual sync call below isn't rejected by the per-user cooldown.
    api::db::integration_tokens::update_last_synced(&app.pool, user_id, "garmin")
        .await
        .unwrap();
    backdate_last_attempt(&app.pool, user_id, "garmin").await;
    let (baseline_synced_at, _) = sync_status(&app.pool, user_id, "garmin").await;

    // First sync: sleeps fetch fails -> the whole sync is reported as failed.
    let first = app
        .app
        .clone()
        .oneshot(post_with_auth("/api/v1/integrations/garmin/sync", &token))
        .await
        .unwrap();
    assert_eq!(first.status(), 502);

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

    // Backdate again so the retry below isn't rejected by the cooldown either
    // — the failed attempt above just bumped `updated_at` to now().
    backdate_last_attempt(&app.pool, user_id, "garmin").await;

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

    // Backdated so the first manual sync call below isn't rejected by the
    // per-user cooldown.
    api::db::integration_tokens::update_last_synced(&app.pool, user_id, "oura")
        .await
        .unwrap();
    backdate_last_attempt(&app.pool, user_id, "oura").await;
    let (baseline_synced_at, _) = sync_status(&app.pool, user_id, "oura").await;

    let first = app
        .app
        .clone()
        .oneshot(post_with_auth("/api/v1/integrations/oura/sync", &token))
        .await
        .unwrap();
    assert_eq!(first.status(), 502);

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

    // Backdate again so the retry below isn't rejected by the cooldown either
    // — the failed attempt above just bumped `updated_at` to now().
    backdate_last_attempt(&app.pool, user_id, "oura").await;

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

// ── Dedup (Blocker 1 regression) ─────────────────────────────────────────

/// Two consecutive successful syncs must not double-insert. This is the
/// exact regression this PR fixes: re-syncing the same wearable data used to
/// insert a fresh duplicate row every 15-minute cycle.
#[tokio::test]
async fn garmin_two_consecutive_syncs_do_not_duplicate_records_or_observations() {
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

    let first = app
        .app
        .clone()
        .oneshot(post_with_auth("/api/v1/integrations/garmin/sync", &token))
        .await
        .unwrap();
    assert_eq!(first.status(), 200);
    let first_body = body_json(first).await;
    let first_inserted = first_body["records_inserted"].as_u64().unwrap();
    assert!(first_inserted > 0);

    // `records_inserted` counts both `health_records` rows and the sleep
    // `observations` row together; capture the `health_records` count
    // separately so the post-re-sync comparison below is apples-to-apples.
    let first_health_record_count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM health_records WHERE user_id = $1 AND source = 'garmin'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert!(first_health_record_count.0 > 0);

    // The sync window is `last_synced_at..now`, and `last_synced_at` just
    // advanced to ~now — a second sync one second later would ask Garmin for
    // an empty window and trivially insert nothing, which wouldn't exercise
    // the dedup path at all. Roll the watermark back so the second sync
    // re-fetches (and WireMock re-serves) the exact same data as the first.
    sqlx::query(
        "UPDATE integration_tokens SET last_synced_at = now() - interval '7 days' \
         WHERE user_id = $1 AND source = 'garmin'",
    )
    .bind(user_id)
    .execute(&app.pool)
    .await
    .unwrap();
    backdate_last_attempt(&app.pool, user_id, "garmin").await;

    let second = app
        .app
        .clone()
        .oneshot(post_with_auth("/api/v1/integrations/garmin/sync", &token))
        .await
        .unwrap();
    assert_eq!(second.status(), 200);
    let second_body = body_json(second).await;
    assert_eq!(
        second_body["records_inserted"].as_u64().unwrap(),
        0,
        "re-syncing identical data must insert zero new rows, not duplicates"
    );

    // health_records: exactly one row per (record_type, date) — steps, resting
    // heart rate x2 days, hrv, body_mass, body_fat_percentage.
    let count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM health_records WHERE user_id = $1 AND source = 'garmin'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        count.0, first_health_record_count.0,
        "row count must not grow on re-sync"
    );

    // observations: exactly one sleep row, not two.
    let obs_count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM observations WHERE user_id = $1 AND source = 'garmin' AND type = 'sleep'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        obs_count.0, 1,
        "re-syncing the same night must not insert a second sleep observation"
    );

    // Row-level assertions, including the Garmin grams -> kg weight conversion
    // (fixture: 75200g -> 75.2kg).
    let body_mass = health_record_by_type(&app.pool, user_id, "body_mass").await;
    assert_eq!(body_mass.1, Some(75.2));
    assert_eq!(body_mass.2.as_deref(), Some("kg"));

    let body_fat = health_record_by_type(&app.pool, user_id, "body_fat_percentage").await;
    assert_eq!(body_fat.1, Some(18.2));
    assert_eq!(body_fat.2.as_deref(), Some("%"));

    let hrv = health_record_by_type(&app.pool, user_id, "heart_rate_variability").await;
    assert_eq!(hrv.1, Some(45.0)); // fixture's `lastNight` value
    assert_eq!(hrv.2.as_deref(), Some("ms"));

    let steps: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM health_records WHERE user_id = $1 AND source = 'garmin' AND record_type = 'steps'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(steps.0, 2, "one steps row per day in the two-day fixture");
}

// ── Cooldown (Blocker 5) ──────────────────────────────────────────────────

#[tokio::test]
async fn garmin_manual_sync_cooldown_returns_429_with_retry_after() {
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

    // First sync succeeds and sets a fresh `updated_at`.
    let first = app
        .app
        .clone()
        .oneshot(post_with_auth("/api/v1/integrations/garmin/sync", &token))
        .await
        .unwrap();
    assert_eq!(first.status(), 200);

    // Immediately syncing again (no time elapsed) must be rejected — this
    // protects the shared Garmin app quota from an abusive client loop.
    let second = app
        .app
        .clone()
        .oneshot(post_with_auth("/api/v1/integrations/garmin/sync", &token))
        .await
        .unwrap();
    assert_eq!(second.status(), 429);
    assert!(
        second.headers().get("retry-after").is_some(),
        "429 response must include a Retry-After header"
    );
}

// ── Advisory lock (Blocker 4) ─────────────────────────────────────────────

#[tokio::test]
async fn garmin_concurrent_manual_syncs_only_one_runs() {
    let garmin_mock = MockServer::start().await;

    // A small delay widens the window where both concurrent requests are
    // in-flight, so the second one's advisory-lock attempt actually races
    // the first's held lock instead of finding it already released.
    let delay = std::time::Duration::from_millis(300);
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/dailies"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/garmin/daily-summary.json"))
                .set_delay(delay),
        )
        .mount(&garmin_mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/sleeps"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/garmin/sleep.json"))
                .set_delay(delay),
        )
        .mount(&garmin_mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/hrv"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/garmin/hrv.json"))
                .set_delay(delay),
        )
        .mount(&garmin_mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/wellness-api/rest/bodyComps"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("../fixtures/garmin/body-comp.json"))
                .set_delay(delay),
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

    // Fire both requests concurrently — this is the very first sync attempt
    // for this connection, so the cooldown (which only applies after a prior
    // attempt) doesn't interfere; only the advisory lock decides.
    let app_a = app.app.clone();
    let token_a = token.clone();
    let app_b = app.app.clone();
    let token_b = token.clone();

    let (result_a, result_b) = tokio::join!(
        app_a.oneshot(post_with_auth("/api/v1/integrations/garmin/sync", &token_a)),
        app_b.oneshot(post_with_auth("/api/v1/integrations/garmin/sync", &token_b)),
    );

    let status_a = result_a.unwrap().status();
    let status_b = result_b.unwrap().status();

    let statuses = [status_a, status_b];
    assert_eq!(
        statuses.iter().filter(|s| **s == 200).count(),
        1,
        "exactly one concurrent sync should run to completion, got {statuses:?}"
    );
    assert_eq!(
        statuses.iter().filter(|s| **s == 429).count(),
        1,
        "the loser of the advisory lock race should be told to retry, got {statuses:?}"
    );

    // The winning sync actually did its job.
    let count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM health_records WHERE user_id = $1 AND source = 'garmin'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert!(count.0 > 0);
}

// ── Cancellation mid-flight (Blocker 2) ──────────────────────────────────

/// Reproduces the exact race `spawn`'s loop performs each cycle
/// (`tokio::select!` between `cancel.cancelled()` and `run_sync(..)`) without
/// waiting through the real 15-minute interval. A slow provider must not
/// delay shutdown: cancelling while `run_sync` is in flight drops its future
/// promptly, and the watermark must not have advanced.
#[tokio::test]
async fn garmin_run_sync_is_interrupted_by_cancellation_mid_flight() {
    let garmin_mock = MockServer::start().await;

    // Every fetch takes far longer than the test's cancellation delay below.
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("[]")
                .set_delay(std::time::Duration::from_secs(5)),
        )
        .mount(&garmin_mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.garmin_client_id = Some("test-garmin-key".to_string());
        cfg.garmin_client_secret = Some("test-garmin-secret".to_string());
        cfg.garmin_base_url = Some(garmin_mock.uri());
    })
    .await;

    let (user_id, _token) = common::create_test_user(&app).await;
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

    let pool = app.pool.clone();
    let config = app.config.clone();
    let http_client = reqwest::Client::new();
    let (event_tx, _) = tokio::sync::broadcast::channel(4);
    let cancel = tokio_util::sync::CancellationToken::new();
    let cancel_for_task = cancel.clone();

    let task = tokio::spawn(async move {
        tokio::select! {
            _ = cancel_for_task.cancelled() => Err("cancelled".to_string()),
            result = api::jobs::garmin_sync::run_sync(&pool, &config, &http_client, &event_tx, &cancel_for_task) => result,
        }
    });

    // Cancel well before the mock's 5s delay could resolve.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    cancel.cancel();

    let result = tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .expect("cancellation should let the pass exit promptly, well under the 5s mock delay")
        .expect("task must not panic");
    assert!(result.is_err(), "the cancelled branch should win the race");

    let (last_synced_at, _) = sync_status(&app.pool, user_id, "garmin").await;
    assert!(
        last_synced_at.is_none(),
        "a cancelled-mid-flight sync must not advance the watermark"
    );
}

// ── Not configured (Blocker 6) ────────────────────────────────────────────

#[tokio::test]
async fn garmin_manual_sync_without_server_config_returns_501() {
    // No garmin_client_id/secret set at all — a self-hoster who hasn't
    // configured this integration, as distinct from a user who hasn't
    // connected it (404).
    let app = common::setup().await;
    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(post_with_auth("/api/v1/integrations/garmin/sync", &token))
        .await
        .unwrap();

    assert_eq!(response.status(), 501);
}

// ── Google Calendar manual sync ───────────────────────────────────────────
//
// The sync job's fetch window is always anchored on "now" (a rolling
// ROLLING_WINDOW_DAYS-back / LOOKAHEAD_DAYS-forward window — see
// jobs::google_calendar_sync), not on a fixed calendar date, so these tests
// build event bodies with dates relative to `Utc::now()` rather than a
// static fixture.

const GCAL_ROLLING_WINDOW_DAYS: i64 = 7;
const GCAL_LOOKAHEAD_DAYS: i64 = 1;
/// Number of `calendar_days` rows a full sync always writes: the window is
/// inclusive of both endpoints (7 days back, today, 1 day ahead).
const GCAL_WINDOW_SIZE: u64 = (GCAL_ROLLING_WINDOW_DAYS + GCAL_LOOKAHEAD_DAYS + 1) as u64;

fn gcal_day_offset(offset_days: i64) -> chrono::NaiveDate {
    (Utc::now() + chrono::Duration::days(offset_days)).date_naive()
}

/// Build a synthetic Google Calendar `events.list` response body. Each tuple
/// is `(days_offset_from_today, start_hour_utc, duration_minutes)` — dates
/// are relative to "now" since the sync window always is too.
fn gcal_events_body(events: &[(i64, u32, i64)]) -> String {
    let items: Vec<serde_json::Value> = events
        .iter()
        .map(|(offset, start_hour, duration_minutes)| {
            let day = gcal_day_offset(*offset);
            let start = day.and_hms_opt(*start_hour, 0, 0).unwrap();
            let end = start + chrono::Duration::minutes(*duration_minutes);
            serde_json::json!({
                "start": {"dateTime": format!("{}Z", start.format("%Y-%m-%dT%H:%M:%S"))},
                "end": {"dateTime": format!("{}Z", end.format("%Y-%m-%dT%H:%M:%S"))}
            })
        })
        .collect();
    serde_json::json!({ "items": items }).to_string()
}

/// Mount a mock for the Google Calendar events endpoint that also asserts
/// the exact `fields`/`eventTypes`/`singleEvents` query params the privacy
/// fix requires — if the request ever regresses to fetching unrestricted
/// event bodies, or drops the `eventTypes=default` server-side filter, this
/// mock stops matching and every test using it fails loudly instead of
/// silently accepting a wider request.
async fn mount_google_calendar_body(mock: &MockServer, body: String) {
    Mock::given(method("GET"))
        .and(path("/calendar/v3/calendars/primary/events"))
        .and(query_param("singleEvents", "true"))
        .and(query_param("eventTypes", "default"))
        .and(query_param(
            "fields",
            "items(start(dateTime),end(dateTime),attendees(self,responseStatus)),nextPageToken",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(mock)
        .await;
}

async fn mount_google_calendar(mock: &MockServer) {
    // Two days of timed meetings; every other day in the window implicitly
    // gets a zero row since there are no events for it.
    mount_google_calendar_body(
        mock,
        gcal_events_body(&[(-2, 9, 30), (-2, 10, 60), (-1, 14, 60)]),
    )
    .await;
}

/// Connect Google Calendar with an access token that won't trigger a
/// proactive refresh (`expires_at` an hour out) — most tests below aren't
/// exercising refresh behavior, and a `None`/near-expiry `expires_at` would
/// make the sync job call `google_token_url`, which defaults to the real
/// `https://oauth2.googleapis.com/token` unless a test overrides it.
async fn connect_google_calendar(app: &common::TestApp, user_id: uuid::Uuid) {
    connect_google_calendar_with_expiry(
        app,
        user_id,
        Some(Utc::now() + chrono::Duration::hours(1)),
    )
    .await;
}

async fn connect_google_calendar_with_expiry(
    app: &common::TestApp,
    user_id: uuid::Uuid,
    expires_at: Option<DateTime<Utc>>,
) {
    let key = test_encryption_key();
    api::db::integration_tokens::upsert(
        &app.pool,
        user_id,
        "google_calendar",
        "gcal-access",
        Some("gcal-refresh"),
        expires_at,
        &key,
    )
    .await
    .unwrap();
}

async fn calendar_days_count(pool: &sqlx::PgPool, user_id: uuid::Uuid) -> i64 {
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM calendar_days WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap();
    count
}

async fn calendar_day_row(
    pool: &sqlx::PgPool,
    user_id: uuid::Uuid,
    date: chrono::NaiveDate,
) -> (i32, i32) {
    sqlx::query_as(
        "SELECT meeting_count, meeting_minutes FROM calendar_days WHERE user_id = $1 AND date = $2",
    )
    .bind(user_id)
    .bind(date)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn google_calendar_manual_sync_requires_auth() {
    let app = common::setup().await;

    let response = app
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/integrations/google-calendar/sync")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn google_calendar_manual_sync_without_connection_returns_404() {
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn google_calendar_manual_sync_underscore_alias_reaches_same_handler() {
    // `/integrations/google_calendar/sync` (underscore) is an alias for
    // `/integrations/google-calendar/sync` (hyphen, tested above) — a client
    // building the URL from the `source` id returned by `GET /integrations`
    // (which is always underscore-separated) must land on a real route
    // rather than 404ing on a path that only exists with a hyphen.
    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
    })
    .await;

    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/google_calendar/sync",
            &token,
        ))
        .await
        .unwrap();

    // Same outcome as the hyphenated path for an unconnected user — proves
    // this reaches `google_calendar::sync`, not a 404 from an unmatched route.
    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn google_calendar_manual_sync_without_server_config_returns_501() {
    let app = common::setup().await;
    let (_, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 501);
}

#[tokio::test]
async fn google_calendar_manual_sync_writes_aggregates_and_advances_watermark() {
    let mock = MockServer::start().await;
    mount_google_calendar(&mock).await;

    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
        cfg.google_calendar_api_base_url = Some(mock.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;
    connect_google_calendar(&app, user_id).await;

    let response = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = body_json(response).await;
    assert_eq!(body["source"], "google_calendar");
    // Every day in the rolling window is written, not just days with
    // meetings — see the CRITICAL windowing fix.
    assert_eq!(body["records_inserted"].as_u64().unwrap(), GCAL_WINDOW_SIZE);
    assert_eq!(
        calendar_days_count(&app.pool, user_id).await as u64,
        GCAL_WINDOW_SIZE
    );

    assert_eq!(
        calendar_day_row(&app.pool, user_id, gcal_day_offset(-2)).await,
        (2, 90),
        "30min + 60min meetings"
    );
    assert_eq!(
        calendar_day_row(&app.pool, user_id, gcal_day_offset(-1)).await,
        (1, 60)
    );
    assert_eq!(
        calendar_day_row(&app.pool, user_id, gcal_day_offset(0)).await,
        (0, 0),
        "a day with no meetings must still get an explicit zero row, not be omitted"
    );

    let (last_synced_at, last_sync_error) =
        sync_status(&app.pool, user_id, "google_calendar").await;
    assert!(last_synced_at.is_some());
    assert!(last_sync_error.is_none());
}

#[tokio::test]
async fn google_calendar_manual_sync_failure_leaves_watermark_then_clears_on_success() {
    let mock = MockServer::start().await;
    // First request fails; subsequent requests succeed.
    Mock::given(method("GET"))
        .and(path("/calendar/v3/calendars/primary/events"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&mock)
        .await;
    mount_google_calendar(&mock).await;

    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
        cfg.google_calendar_api_base_url = Some(mock.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;
    connect_google_calendar(&app, user_id).await;

    let first = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(first.status(), 502);

    let (after_failure_synced_at, after_failure_error) =
        sync_status(&app.pool, user_id, "google_calendar").await;
    assert!(
        after_failure_synced_at.is_none(),
        "watermark must not advance when the fetch fails"
    );
    assert!(
        after_failure_error.is_some(),
        "last_sync_error must be recorded on failure"
    );
    assert_eq!(
        calendar_days_count(&app.pool, user_id).await,
        0,
        "a failed sync must not write partial aggregates"
    );

    backdate_last_attempt(&app.pool, user_id, "google_calendar").await;

    let second = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(second.status(), 200);

    let (after_success_synced_at, after_success_error) =
        sync_status(&app.pool, user_id, "google_calendar").await;
    assert!(after_success_synced_at.is_some());
    assert!(after_success_error.is_none());
    assert_eq!(
        calendar_days_count(&app.pool, user_id).await as u64,
        GCAL_WINDOW_SIZE
    );
}

#[tokio::test]
async fn google_calendar_re_sync_overwrites_rather_than_duplicates() {
    let mock = MockServer::start().await;
    mount_google_calendar(&mock).await;

    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
        cfg.google_calendar_api_base_url = Some(mock.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;
    connect_google_calendar(&app, user_id).await;

    let first = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(first.status(), 200);
    assert_eq!(
        calendar_days_count(&app.pool, user_id).await as u64,
        GCAL_WINDOW_SIZE
    );

    // No watermark rollback needed — the window is always anchored on "now",
    // never on `last_synced_at` (that's the CRITICAL fix), so a second sync
    // re-covers the exact same days regardless of when the first ran.
    backdate_last_attempt(&app.pool, user_id, "google_calendar").await;

    let second = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(second.status(), 200);

    assert_eq!(
        calendar_days_count(&app.pool, user_id).await as u64,
        GCAL_WINDOW_SIZE,
        "re-syncing identical data must overwrite existing rows, not add new ones"
    );
    assert_eq!(
        calendar_day_row(&app.pool, user_id, gcal_day_offset(-2)).await,
        (2, 90),
        "values must be recomputed from source data, not accumulated"
    );
}

/// The regression this fix targets: a day already in the past must be
/// corrected if a meeting on it is later cancelled — not stuck at whatever
/// the first sync saw, which is what happened when the fetch window was
/// anchored on `last_synced_at` instead of always rolling.
#[tokio::test]
async fn google_calendar_re_sync_corrects_day_when_meeting_cancelled() {
    let mock = MockServer::start().await;
    // First sync: two meetings on day -2.
    Mock::given(method("GET"))
        .and(path("/calendar/v3/calendars/primary/events"))
        .and(query_param("singleEvents", "true"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(gcal_events_body(&[(-2, 9, 30), (-2, 10, 60)])),
        )
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&mock)
        .await;
    // Second sync: one of those meetings was cancelled upstream.
    Mock::given(method("GET"))
        .and(path("/calendar/v3/calendars/primary/events"))
        .and(query_param("singleEvents", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_string(gcal_events_body(&[(-2, 9, 30)])))
        .with_priority(2)
        .mount(&mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
        cfg.google_calendar_api_base_url = Some(mock.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;
    connect_google_calendar(&app, user_id).await;

    let first = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(first.status(), 200);
    assert_eq!(
        calendar_day_row(&app.pool, user_id, gcal_day_offset(-2)).await,
        (2, 90)
    );

    backdate_last_attempt(&app.pool, user_id, "google_calendar").await;

    let second = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(second.status(), 200);
    assert_eq!(
        calendar_day_row(&app.pool, user_id, gcal_day_offset(-2)).await,
        (1, 30),
        "the day must correct down to reflect the cancelled meeting, not stay \
         stuck at the first sync's count"
    );
}

#[tokio::test]
async fn google_calendar_manual_sync_cooldown_returns_429_with_retry_after() {
    let mock = MockServer::start().await;
    mount_google_calendar(&mock).await;

    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
        cfg.google_calendar_api_base_url = Some(mock.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;
    connect_google_calendar(&app, user_id).await;

    let first = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(first.status(), 200);

    let second = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(second.status(), 429);
    assert!(second.headers().get("retry-after").is_some());
}

#[tokio::test]
async fn google_calendar_concurrent_manual_syncs_only_one_runs() {
    let mock = MockServer::start().await;
    let delay = std::time::Duration::from_millis(300);
    Mock::given(method("GET"))
        .and(path("/calendar/v3/calendars/primary/events"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(gcal_events_body(&[(-2, 9, 30), (-2, 10, 60), (-1, 14, 60)]))
                .set_delay(delay),
        )
        .mount(&mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
        cfg.google_calendar_api_base_url = Some(mock.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;
    connect_google_calendar(&app, user_id).await;

    let app_a = app.app.clone();
    let token_a = token.clone();
    let app_b = app.app.clone();
    let token_b = token.clone();

    let (result_a, result_b) = tokio::join!(
        app_a.oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token_a
        )),
        app_b.oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token_b
        )),
    );

    let statuses = [result_a.unwrap().status(), result_b.unwrap().status()];
    assert_eq!(
        statuses.iter().filter(|s| **s == 200).count(),
        1,
        "exactly one concurrent sync should run to completion, got {statuses:?}"
    );
    assert_eq!(
        statuses.iter().filter(|s| **s == 429).count(),
        1,
        "the loser of the advisory lock race should be told to retry, got {statuses:?}"
    );

    assert_eq!(
        calendar_days_count(&app.pool, user_id).await as u64,
        GCAL_WINDOW_SIZE
    );
}

#[tokio::test]
async fn google_calendar_run_sync_is_interrupted_by_cancellation_mid_flight() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("{\"items\": []}")
                .set_delay(std::time::Duration::from_secs(5)),
        )
        .mount(&mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
        cfg.google_calendar_api_base_url = Some(mock.uri());
    })
    .await;

    let (user_id, _token) = common::create_test_user(&app).await;
    connect_google_calendar(&app, user_id).await;

    let pool = app.pool.clone();
    let config = app.config.clone();
    let http_client = reqwest::Client::new();
    let (event_tx, _) = tokio::sync::broadcast::channel(4);
    let cancel = tokio_util::sync::CancellationToken::new();
    let cancel_for_task = cancel.clone();

    let task = tokio::spawn(async move {
        tokio::select! {
            _ = cancel_for_task.cancelled() => Err("cancelled".to_string()),
            result = api::jobs::google_calendar_sync::run_sync(&pool, &config, &http_client, &event_tx, &cancel_for_task) => result,
        }
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    cancel.cancel();

    let result = tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .expect("cancellation should let the pass exit promptly, well under the 5s mock delay")
        .expect("task must not panic");
    assert!(result.is_err(), "the cancelled branch should win the race");

    let (last_synced_at, _) = sync_status(&app.pool, user_id, "google_calendar").await;
    assert!(
        last_synced_at.is_none(),
        "a cancelled-mid-flight sync must not advance the watermark"
    );
}

#[tokio::test]
async fn google_calendar_refreshes_when_expires_at_is_none() {
    let mock = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "refreshed-access-token",
            "expires_in": 3600
        })))
        .mount(&mock)
        .await;

    // Only a request bearing the *refreshed* token succeeds — proves the
    // job refreshed before fetching rather than using the stale token.
    Mock::given(method("GET"))
        .and(path("/calendar/v3/calendars/primary/events"))
        .and(header("authorization", "Bearer refreshed-access-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string(gcal_events_body(&[])))
        .mount(&mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
        cfg.google_calendar_api_base_url = Some(mock.uri());
        cfg.google_token_url = format!("{}/token", mock.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;
    connect_google_calendar_with_expiry(&app, user_id, None).await;

    let response = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        200,
        "a token with no expires_at must be treated as due for refresh, not never refreshed"
    );

    let key = test_encryption_key();
    let stored = api::db::integration_tokens::list_for_user(&app.pool, user_id, &key, None)
        .await
        .unwrap()
        .into_iter()
        .find(|t| t.source == "google_calendar")
        .unwrap();
    assert_eq!(stored.access_token, "refreshed-access-token");
}

#[tokio::test]
async fn google_calendar_retries_once_after_401() {
    let mock = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": "recovered-access-token",
            "expires_in": 3600
        })))
        .mount(&mock)
        .await;

    // The original (not-yet-expired, per `expires_at`) token is rejected...
    Mock::given(method("GET"))
        .and(path("/calendar/v3/calendars/primary/events"))
        .and(header("authorization", "Bearer gcal-access"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&mock)
        .await;
    // ...but a retry with the refreshed token succeeds.
    Mock::given(method("GET"))
        .and(path("/calendar/v3/calendars/primary/events"))
        .and(header("authorization", "Bearer recovered-access-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string(gcal_events_body(&[])))
        .mount(&mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
        cfg.google_calendar_api_base_url = Some(mock.uri());
        cfg.google_token_url = format!("{}/token", mock.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;
    // expires_at far in the future — only the reactive 401 handler should
    // trigger a refresh here, not the proactive expiry check.
    connect_google_calendar_with_expiry(
        &app,
        user_id,
        Some(Utc::now() + chrono::Duration::hours(1)),
    )
    .await;

    let response = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        200,
        "a 401 despite a not-yet-expired token must trigger exactly one refresh-and-retry"
    );
}

#[tokio::test]
async fn google_calendar_excludes_declined_meetings_end_to_end() {
    let mock = MockServer::start().await;
    let day = gcal_day_offset(-1);
    let body = serde_json::json!({
        "items": [
            {
                "start": {"dateTime": format!("{day}T09:00:00Z")},
                "end": {"dateTime": format!("{day}T09:30:00Z")},
                "attendees": [{"self": true, "responseStatus": "declined"}]
            },
            {
                "start": {"dateTime": format!("{day}T10:00:00Z")},
                "end": {"dateTime": format!("{day}T10:30:00Z")}
            }
        ]
    })
    .to_string();
    mount_google_calendar_body(&mock, body).await;

    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
        cfg.google_calendar_api_base_url = Some(mock.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;
    connect_google_calendar(&app, user_id).await;

    let response = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    assert_eq!(
        calendar_day_row(&app.pool, user_id, day).await,
        (1, 30),
        "the declined meeting must not count, only the accepted one"
    );
}

#[tokio::test]
async fn google_calendar_uses_rolling_window_anchored_on_now_not_last_synced_at() {
    let mock = MockServer::start().await;
    let expected_time_min = gcal_day_offset(-GCAL_ROLLING_WINDOW_DAYS)
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .to_rfc3339();

    // Only matches if `timeMin` is the fixed 7-day-back day boundary — if
    // the window were still anchored on `last_synced_at` (an instant, not a
    // day boundary), this request wouldn't match and the sync would fail
    // upstream with an unmatched-mock 404.
    Mock::given(method("GET"))
        .and(path("/calendar/v3/calendars/primary/events"))
        .and(query_param("timeMin", expected_time_min.as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_string(gcal_events_body(&[])))
        .mount(&mock)
        .await;

    let app = common::setup_with_config(|cfg| {
        cfg.google_client_id = Some("test-google-id".to_string());
        cfg.google_client_secret = Some("test-google-secret".to_string());
        cfg.google_calendar_api_base_url = Some(mock.uri());
    })
    .await;

    let (user_id, token) = common::create_test_user(&app).await;
    connect_google_calendar(&app, user_id).await;
    // Simulate "already synced a moment ago" — under the old (buggy)
    // behavior this would anchor `timeMin` on this recent instant instead of
    // the fixed 7-day-back boundary, and the mock above wouldn't match.
    api::db::integration_tokens::update_last_synced(&app.pool, user_id, "google_calendar")
        .await
        .unwrap();
    backdate_last_attempt(&app.pool, user_id, "google_calendar").await;

    let response = app
        .app
        .clone()
        .oneshot(post_with_auth(
            "/api/v1/integrations/google-calendar/sync",
            &token,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}
