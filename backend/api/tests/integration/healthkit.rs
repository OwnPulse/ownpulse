// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Integration tests for `POST /api/v1/healthkit/sync` — the HealthKit bulk
//! sync endpoint used by the iOS client. Verifies set-based UNNEST insert
//! behaviour, idempotency on the (user, source, record_type, start_time,
//! source_id) unique constraint, and the required invariants for HealthKit
//! data (source forced to `"healthkit"`, `duplicate_of` always NULL).
//!
//! Cross-source dedup is **deliberately not tested here**. That logic used to
//! run inline on the sync path, making 100-record batches take ~1s with 200
//! DB round trips. It was moved out of the sync path entirely in
//! `perf/healthkit-bulk-insert`; re-adding it is tracked as a deferred async
//! reconciliation job.

use serde_json::json;
use tower::ServiceExt;

use crate::common;

/// A batch of 3 new records inserts 3 rows.
#[tokio::test]
async fn test_healthkit_sync_inserts_new_records() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "records": [
            {
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 72.0,
                "unit": "bpm",
                "start_time": "2026-04-17T10:00:00Z",
                "source_id": "hk-uuid-1"
            },
            {
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 74.0,
                "unit": "bpm",
                "start_time": "2026-04-17T10:01:00Z",
                "source_id": "hk-uuid-2"
            },
            {
                "source": "healthkit",
                "record_type": "steps",
                "value": 1200.0,
                "unit": "count",
                "start_time": "2026-04-17T10:02:00Z",
                "source_id": "hk-uuid-3"
            }
        ]
    });

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/sync",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 201);

    // Verify three rows landed in the DB, all with source='healthkit' and
    // duplicate_of IS NULL.
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM health_records
         WHERE user_id = $1 AND source = 'healthkit' AND duplicate_of IS NULL",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(count.0, 3);
}

/// A batch replayed identically inserts 0 new rows (ON CONFLICT DO NOTHING).
#[tokio::test]
async fn test_healthkit_sync_is_idempotent_on_replay() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "records": [
            {
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 72.0,
                "unit": "bpm",
                "start_time": "2026-04-17T10:00:00Z",
                "source_id": "replay-1"
            },
            {
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 74.0,
                "unit": "bpm",
                "start_time": "2026-04-17T10:01:00Z",
                "source_id": "replay-2"
            }
        ]
    });

    // First POST — 2 new rows.
    let resp1 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/sync",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp1.status(), 201);

    // Second POST with the exact same body — no new rows.
    let resp2 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/sync",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp2.status(), 201);

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM health_records WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(count.0, 2);
}

/// A batch where one record has a duplicate `source_id` with an existing row
/// is deduped (count == N - 1).
#[tokio::test]
async fn test_healthkit_sync_dedups_partial_overlap() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    // Seed one existing row directly in the DB.
    sqlx::query(
        "INSERT INTO health_records
            (user_id, source, record_type, value, unit, start_time, source_id)
         VALUES ($1, 'healthkit', 'heart_rate', 72.0, 'bpm',
                 '2026-04-17T10:00:00Z', 'seed-existing')",
    )
    .bind(user_id)
    .execute(&app.pool)
    .await
    .unwrap();

    // Post a batch of 3 where the middle record collides with the seeded row.
    let body = json!({
        "records": [
            {
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 70.0,
                "unit": "bpm",
                "start_time": "2026-04-17T09:59:00Z",
                "source_id": "fresh-1"
            },
            {
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 72.0,
                "unit": "bpm",
                "start_time": "2026-04-17T10:00:00Z",
                "source_id": "seed-existing"
            },
            {
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 74.0,
                "unit": "bpm",
                "start_time": "2026-04-17T10:01:00Z",
                "source_id": "fresh-2"
            }
        ]
    });

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/sync",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 201);

    // 1 seeded + 2 fresh = 3 total (the colliding record is dropped by
    // ON CONFLICT DO NOTHING).
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM health_records WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(count.0, 3);
}

/// The route forces `source = 'healthkit'` in the SQL regardless of what the
/// client sends — any attempt to smuggle a different source is ignored.
#[tokio::test]
async fn test_healthkit_sync_forces_source_healthkit() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "records": [
            {
                "source": "garmin",
                "record_type": "heart_rate",
                "value": 72.0,
                "unit": "bpm",
                "start_time": "2026-04-17T10:00:00Z",
                "source_id": "spoof-1"
            }
        ]
    });

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/sync",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 201);

    let row: (String,) = sqlx::query_as("SELECT source FROM health_records WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(row.0, "healthkit");
}

/// An empty batch returns 201 and inserts nothing — no SQL is executed.
#[tokio::test]
async fn test_healthkit_sync_empty_batch() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    let body = json!({ "records": [] });

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/sync",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 201);

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM health_records WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
}

/// Unauthenticated requests are rejected with 401.
#[tokio::test]
async fn test_healthkit_sync_unauthenticated() {
    let app = common::setup().await;

    let body = json!({
        "records": [
            {
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 72.0,
                "unit": "bpm",
                "start_time": "2026-04-17T10:00:00Z",
                "source_id": "noauth-1"
            }
        ]
    });

    let request = http::Request::builder()
        .method("POST")
        .uri("/api/v1/healthkit/sync")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::to_string(&body).unwrap(),
        ))
        .unwrap();

    let response = app.app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 401);
}

/// Malformed JSON body is rejected with 400 (or 422 for invalid shape) —
/// never 500.
#[tokio::test]
async fn test_healthkit_sync_invalid_body() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Missing required `records` field.
    let body = json!({ "not_records": [] });

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/sync",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    let status = response.status().as_u16();
    assert!(
        (400..500).contains(&status),
        "expected 4xx for malformed body, got {status}"
    );
}
