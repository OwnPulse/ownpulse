// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Integration tests for `POST /api/v1/healthkit/sync` — the HealthKit bulk
//! sync endpoint used by the iOS client. Verifies set-based UNNEST insert
//! behaviour, idempotency on the (user, source, record_type, start_time,
//! source_id) unique constraint, and the required invariants for HealthKit
//! data (source forced to `"healthkit"`, cross-source dedup via
//! `duplicate_of`).
//!
//! Cross-source dedup is preserved as a two-query batched path (preflight
//! SELECT + single INSERT). Per-record `find_duplicate` loops were removed
//! for performance but the deduplication rule in `CLAUDE.md` still holds —
//! see `test_healthkit_sync_cross_source_dedup_bulk` below.

use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

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

/// Cross-source dedup happens inside the bulk path:
///
/// - A pre-existing Garmin heart_rate row at `07:01:15Z` (value 58.0) gets
///   matched by a healthkit heart_rate at `07:01:16Z` (value 58.3 — within
///   the 2% tolerance), so the new row is inserted with `duplicate_of` set
///   to the Garmin row's id.
/// - A second healthkit row at the same timestamp but value 70.0 is outside
///   the tolerance, so it is inserted with `duplicate_of IS NULL`.
///
/// The Garmin row is preserved verbatim — dedup never silently drops data.
#[tokio::test]
async fn test_healthkit_sync_cross_source_dedup_bulk() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    // Seed a Garmin-sourced row: heart_rate 58 bpm at 07:01:15Z.
    let garmin_id: Uuid = sqlx::query_scalar(
        "INSERT INTO health_records
            (user_id, source, record_type, value, unit, start_time, source_id)
         VALUES ($1, 'garmin', 'heart_rate', 58.0, 'bpm',
                 '2026-04-18T07:01:15Z', 'garmin-xyz')
         RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    // Batch:
    // - hk-within: 07:01:16Z, value 58.3 -> within 2% tolerance of Garmin 58.
    // - hk-outside: 07:01:15Z, value 70.0 -> outside 2% tolerance; separate source_id
    //   so the UNIQUE(user,source,record_type,start_time,source_id) constraint
    //   lets both healthkit rows land at the same start_time.
    let body = json!({
        "records": [
            {
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 58.3,
                "unit": "bpm",
                "start_time": "2026-04-18T07:01:16Z",
                "source_id": "hk-abc"
            },
            {
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 70.0,
                "unit": "bpm",
                "start_time": "2026-04-18T07:01:15Z",
                "source_id": "hk-far"
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

    // Garmin row still there, untouched.
    let garmin_exists: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM health_records
         WHERE id = $1 AND source = 'garmin' AND duplicate_of IS NULL",
    )
    .bind(garmin_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(garmin_exists.0, 1, "Garmin row must be preserved");

    // hk-within: inserted with duplicate_of = garmin row id.
    let within_dup_of: (Option<Uuid>,) = sqlx::query_as(
        "SELECT duplicate_of FROM health_records
         WHERE user_id = $1 AND source = 'healthkit' AND source_id = 'hk-abc'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        within_dup_of.0,
        Some(garmin_id),
        "healthkit row within 2% tolerance must point at Garmin row via duplicate_of"
    );

    // hk-outside: inserted with duplicate_of = NULL (70.0 is not within 2% of 58.0).
    let outside_dup_of: (Option<Uuid>,) = sqlx::query_as(
        "SELECT duplicate_of FROM health_records
         WHERE user_id = $1 AND source = 'healthkit' AND source_id = 'hk-far'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert!(
        outside_dup_of.0.is_none(),
        "healthkit row outside 2% tolerance must NOT be flagged as duplicate"
    );

    // Total health_records count for this user: 1 Garmin + 2 healthkit = 3.
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM health_records WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(total.0, 3);
}

/// Submitting records with `source` values other than `"healthkit"` must not
/// smuggle foreign sources past the handler. We verify belt-and-braces
/// behaviour: the route mutates `source` on ingress, and the SQL projection
/// hard-codes `'healthkit'` as a literal — so all three rows land with
/// `source = 'healthkit'` regardless of what the client sent.
#[tokio::test]
async fn test_healthkit_sync_mixed_sources_all_forced_to_healthkit() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "records": [
            {
                "source": "garmin",
                "record_type": "heart_rate",
                "value": 70.0,
                "unit": "bpm",
                "start_time": "2026-04-18T08:00:00Z",
                "source_id": "mixed-garmin"
            },
            {
                "source": "manual",
                "record_type": "heart_rate",
                "value": 72.0,
                "unit": "bpm",
                "start_time": "2026-04-18T08:01:00Z",
                "source_id": "mixed-manual"
            },
            {
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 74.0,
                "unit": "bpm",
                "start_time": "2026-04-18T08:02:00Z",
                "source_id": "mixed-hk"
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

    // All three source_ids landed and every row carries source = 'healthkit'.
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT source, source_id FROM health_records
         WHERE user_id = $1 ORDER BY source_id",
    )
    .bind(user_id)
    .fetch_all(&app.pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 3);
    for (source, source_id) in &rows {
        assert_eq!(
            source, "healthkit",
            "row source_id={source_id} landed with source={source}, expected 'healthkit'"
        );
    }
}

/// Two identical rows in the same batch must not raise a unique-violation
/// error — `ON CONFLICT DO NOTHING` on the UNIQUE constraint swallows the
/// second. Postgres applies conflict resolution per-tuple on
/// `INSERT ... SELECT`, so in-statement collisions on the conflict target
/// are handled the same as cross-statement ones.
#[tokio::test]
async fn test_healthkit_sync_in_batch_duplicates() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    // Exactly the same (source_id, start_time, record_type) tuple twice.
    let body = json!({
        "records": [
            {
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 80.0,
                "unit": "bpm",
                "start_time": "2026-04-18T09:00:00Z",
                "source_id": "in-batch-dup"
            },
            {
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 80.0,
                "unit": "bpm",
                "start_time": "2026-04-18T09:00:00Z",
                "source_id": "in-batch-dup"
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
    assert_eq!(
        response.status(),
        201,
        "duplicate rows inside a single batch must be handled by ON CONFLICT DO NOTHING, not error out"
    );

    // Only one row landed.
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM health_records
         WHERE user_id = $1 AND source_id = 'in-batch-dup'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(count.0, 1);
}

/// `source_id IS NULL` is treated as distinct by the UNIQUE constraint
/// (Postgres default behaviour). Two records with `source_id: None` and
/// otherwise identical payload both land. This is **not** a nice invariant
/// for idempotent sync, but the schema's UNIQUE index is nullable-column
/// permissive and we document it here so nobody's surprised — if we ever want
/// NULL-treated-as-equal semantics, that needs a schema-level partial unique
/// index, not a hack in this handler.
#[tokio::test]
async fn test_healthkit_sync_null_source_id_does_not_dedup() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    // Two records, both source_id = null, same (record_type, start_time, value).
    let body = json!({
        "records": [
            {
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 90.0,
                "unit": "bpm",
                "start_time": "2026-04-18T10:00:00Z",
                "source_id": null
            },
            {
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 90.0,
                "unit": "bpm",
                "start_time": "2026-04-18T10:00:00Z",
                "source_id": null
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

    // Both land — NULL source_id does not participate in the unique index
    // equality check.
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM health_records
         WHERE user_id = $1 AND source_id IS NULL",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        count.0, 2,
        "two NULL-source_id rows should both land — Postgres UNIQUE treats NULL as distinct"
    );
}

/// Batches larger than `MAX_HEALTHKIT_BATCH` (500) are rejected with 400 before
/// touching the DB. Prevents a pathological client from starving the pool or
/// blowing up process memory on per-record array allocations.
#[tokio::test]
async fn test_healthkit_sync_rejects_oversized_batch() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    // 501 records — one over the ceiling.
    let records: Vec<_> = (0..501)
        .map(|i| {
            json!({
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 72.0,
                "unit": "bpm",
                // Each start_time staggered by 1 second so that, if the route
                // *did* let this through, records would all pass the unique
                // constraint and land — guaranteeing a visible 500/201
                // mismatch vs. the expected 400.
                "start_time": format!("2026-04-18T11:{:02}:{:02}Z", i / 60, i % 60),
                "source_id": format!("oversize-{i}")
            })
        })
        .collect();
    let body = json!({ "records": records });

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
    assert_eq!(
        response.status(),
        400,
        "batches over MAX_HEALTHKIT_BATCH must be rejected before reaching the DB"
    );

    // And no rows landed.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM health_records WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
}
