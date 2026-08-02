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
