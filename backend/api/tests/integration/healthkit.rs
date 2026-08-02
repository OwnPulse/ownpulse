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

// ============================================================================
// GET /healthkit/write-queue + POST /healthkit/confirm
// ============================================================================

/// Posting a HK-mapped manual health record enqueues a write-queue item whose
/// `value` JSONB has exactly the {value, unit, start_time, end_time} shape —
/// this is the iOS decode contract and, before this test, was never pinned.
#[tokio::test]
async fn test_write_queue_shape_after_manual_record_insert() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "source": "manual",
        "record_type": "body_mass",
        "value": 82.5,
        "unit": "kg",
        "start_time": "2026-04-19T08:00:00Z",
        "end_time": "2026-04-19T08:00:00Z"
    });

    let create_response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/health-records",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(create_response.status(), 201);

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/healthkit/write-queue",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let queue = common::body_json(response).await;
    let items = queue.as_array().expect("array response");
    assert_eq!(
        items.len(),
        1,
        "manual record must enqueue exactly one item"
    );

    let item = &items[0];
    assert_eq!(item["hk_type"], "body_mass");
    assert_eq!(item["user_id"], user_id.to_string());

    // Pin the `value` JSONB shape key-by-key — this is the iOS decode contract.
    let value = item["value"]
        .as_object()
        .expect("value must be a JSON object");
    let mut keys: Vec<&str> = value.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["end_time", "start_time", "unit", "value"],
        "write-queue value JSONB must have exactly these keys"
    );
    assert_eq!(value["value"], 82.5);
    assert_eq!(value["unit"], "kg");
    assert_eq!(value["start_time"], "2026-04-19T08:00:00Z");
    assert_eq!(value["end_time"], "2026-04-19T08:00:00Z");
}

/// Every inner field of the write-queue `value` JSONB except `start_time` is
/// nullable on the decode contract: `HealthRecordRow.value`/`unit`/`end_time`
/// are all `Option` in the DB model, and a manual record posted with none of
/// them still enqueues — with those keys present and `null`, not omitted.
/// (iOS is being told separately to decode these as `Double?`/`String?`/
/// `Date?` and fail-report null-value items rather than crash.)
#[tokio::test]
async fn test_write_queue_shape_with_null_value_fields() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "source": "manual",
        "record_type": "workout",
        "value": null,
        "unit": null,
        "start_time": "2026-04-19T08:00:00Z",
        "end_time": null
    });

    let create_response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/health-records",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(create_response.status(), 201);

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/healthkit/write-queue",
            &token,
            None,
        ))
        .await
        .unwrap();
    let queue = common::body_json(response).await;
    let items = queue.as_array().expect("array response");
    assert_eq!(items.len(), 1);

    let value = items[0]["value"]
        .as_object()
        .expect("value must be a JSON object");
    let mut keys: Vec<&str> = value.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["end_time", "start_time", "unit", "value"],
        "keys must be present even when null, not omitted"
    );
    assert!(value["value"].is_null());
    assert!(value["unit"].is_null());
    assert_eq!(value["start_time"], "2026-04-19T08:00:00Z");
    assert!(value["end_time"].is_null());
}

/// `POST /healthkit/sync` on an all-duplicate (0-inserted) batch must not
/// publish a `health_records` SSE event — iOS polls this endpoint frequently
/// even with nothing new to sync, and every dashboard query refetching on a
/// zero-row batch defeats the point of the event (signal, not noise).
#[tokio::test]
async fn test_healthkit_sync_all_duplicate_batch_does_not_publish_event() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let mut receiver = app.event_tx.subscribe();

    let body = json!({
        "records": [
            {
                "source": "healthkit",
                "record_type": "heart_rate",
                "value": 72.0,
                "unit": "bpm",
                "start_time": "2026-04-20T10:00:00Z",
                "source_id": "replay-event-check"
            }
        ]
    });

    // First sync — inserts one row and (correctly) publishes.
    let first = app
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
    assert_eq!(first.status(), 201);
    receiver
        .try_recv()
        .expect("first sync inserts a row and must publish an event");

    // Replay the identical batch — 0 rows inserted (ON CONFLICT DO NOTHING).
    let second = app
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
    assert_eq!(second.status(), 201);

    assert!(
        receiver.try_recv().is_err(),
        "an all-duplicate (0-inserted) batch must not publish an event"
    );
}

/// Within a single `POST /healthkit/confirm` request, an id present in both
/// `ids` and `failures` resolves to confirmed — `confirm` wins over a
/// same-request failure report.
#[tokio::test]
async fn test_confirm_wins_over_failure_in_same_request() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    let queue_id: Uuid = sqlx::query_scalar(
        "INSERT INTO healthkit_write_queue (user_id, hk_type, value)
         VALUES ($1, 'body_mass', '{\"value\": 80.0, \"unit\": \"kg\"}'::jsonb)
         RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    let body = json!({
        "ids": [queue_id],
        "failures": [{ "id": queue_id, "error": "reported failed and confirmed in one request" }]
    });
    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/confirm",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 204);

    let row: (
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as("SELECT confirmed_at, failed_at FROM healthkit_write_queue WHERE id = $1")
        .bind(queue_id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert!(row.0.is_some(), "confirmed_at must be set — confirm wins");
    assert!(
        row.1.is_none(),
        "failed_at must NOT be set when the same id is also confirmed"
    );
}

/// An id already marked failed by an earlier request stays failed if a later,
/// unrelated request lists it in `ids` (e.g. a stale client retry queued
/// before the failure was reported) — `confirm`'s guard excludes rows with
/// `failed_at` already set.
#[tokio::test]
async fn test_previously_failed_id_stays_failed_on_later_confirm() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    let queue_id: Uuid = sqlx::query_scalar(
        "INSERT INTO healthkit_write_queue (user_id, hk_type, value)
         VALUES ($1, 'body_mass', '{\"value\": 80.0, \"unit\": \"kg\"}'::jsonb)
         RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    // First request: report it failed.
    let fail_body = json!({
        "ids": [],
        "failures": [{ "id": queue_id, "error": "HealthKit authorization denied" }]
    });
    let fail_response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/confirm",
            &token,
            Some(&fail_body),
        ))
        .await
        .unwrap();
    assert_eq!(fail_response.status(), 204);

    // Second, later request: a stale retry lists the same id as confirmed.
    let confirm_body = json!({ "ids": [queue_id] });
    let confirm_response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/confirm",
            &token,
            Some(&confirm_body),
        ))
        .await
        .unwrap();
    assert_eq!(confirm_response.status(), 204);

    let row: (
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as("SELECT confirmed_at, failed_at FROM healthkit_write_queue WHERE id = $1")
        .bind(queue_id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert!(
        row.1.is_some(),
        "failed_at must remain set — a later confirm must not override an earlier failure"
    );
    assert!(
        row.0.is_none(),
        "confirmed_at must stay unset for a row already marked failed"
    );
}

/// Two failures reported for the same id in one request must not error or
/// store a nondeterministic result — the last occurrence in the array wins.
#[tokio::test]
async fn test_confirm_dedupes_repeated_failure_id_keeping_last_error() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    let queue_id: Uuid = sqlx::query_scalar(
        "INSERT INTO healthkit_write_queue (user_id, hk_type, value)
         VALUES ($1, 'body_mass', '{\"value\": 80.0, \"unit\": \"kg\"}'::jsonb)
         RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    let body = json!({
        "ids": [],
        "failures": [
            { "id": queue_id, "error": "first report" },
            { "id": queue_id, "error": "second report" }
        ]
    });
    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/confirm",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        204,
        "a duplicate id within failures must not error"
    );

    let row: (Option<String>,) =
        sqlx::query_as("SELECT error FROM healthkit_write_queue WHERE id = $1")
            .bind(queue_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(
        row.0.as_deref(),
        Some("second report"),
        "the last occurrence of a duplicate id must win deterministically"
    );
}

/// `POST /healthkit/confirm` with `ids` marks matching rows confirmed and
/// they no longer appear in the pending write-queue.
#[tokio::test]
async fn test_confirm_marks_ids_confirmed_and_removes_from_pending() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    let queue_id: Uuid = sqlx::query_scalar(
        "INSERT INTO healthkit_write_queue (user_id, hk_type, value)
         VALUES ($1, 'body_mass', '{\"value\": 80.0, \"unit\": \"kg\"}'::jsonb)
         RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    let body = json!({ "ids": [queue_id] });
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/confirm",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 204);

    let row: (Option<chrono::DateTime<chrono::Utc>>,) =
        sqlx::query_as("SELECT confirmed_at FROM healthkit_write_queue WHERE id = $1")
            .bind(queue_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert!(row.0.is_some(), "confirmed_at must be set");

    let pending_response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/healthkit/write-queue",
            &token,
            None,
        ))
        .await
        .unwrap();
    let pending = common::body_json(pending_response).await;
    assert_eq!(pending.as_array().unwrap().len(), 0);
}

/// `POST /healthkit/confirm` with `failures` sets `failed_at` and `error` on
/// the matching rows, and they no longer appear in the pending write-queue —
/// unblocking the `LIMIT 100` head-of-line problem a permanently-failing item
/// would otherwise cause.
#[tokio::test]
async fn test_confirm_with_failures_marks_failed_and_removes_from_pending() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    let queue_id: Uuid = sqlx::query_scalar(
        "INSERT INTO healthkit_write_queue (user_id, hk_type, value)
         VALUES ($1, 'body_mass', '{\"value\": 80.0, \"unit\": \"kg\"}'::jsonb)
         RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    let body = json!({
        "ids": [],
        "failures": [
            { "id": queue_id, "error": "HealthKit authorization denied for Body Mass" }
        ]
    });
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/confirm",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 204);

    let row: (Option<chrono::DateTime<chrono::Utc>>, Option<String>) =
        sqlx::query_as("SELECT failed_at, error FROM healthkit_write_queue WHERE id = $1")
            .bind(queue_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert!(row.0.is_some(), "failed_at must be set");
    assert_eq!(
        row.1.as_deref(),
        Some("HealthKit authorization denied for Body Mass")
    );

    let pending_response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/healthkit/write-queue",
            &token,
            None,
        ))
        .await
        .unwrap();
    let pending = common::body_json(pending_response).await;
    assert_eq!(
        pending.as_array().unwrap().len(),
        0,
        "failed items must not remain pending"
    );
}

/// Error strings longer than 500 chars are truncated before storage — the
/// client controls this text and it must not grow the column unbounded.
#[tokio::test]
async fn test_confirm_with_failures_truncates_long_error() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    let queue_id: Uuid = sqlx::query_scalar(
        "INSERT INTO healthkit_write_queue (user_id, hk_type, value)
         VALUES ($1, 'body_mass', '{\"value\": 80.0, \"unit\": \"kg\"}'::jsonb)
         RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    let long_error = "x".repeat(1000);
    let body = json!({
        "ids": [],
        "failures": [{ "id": queue_id, "error": long_error }]
    });
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/confirm",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 204);

    let row: (Option<String>,) =
        sqlx::query_as("SELECT error FROM healthkit_write_queue WHERE id = $1")
            .bind(queue_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(row.0.map(|e| e.len()), Some(500));
}

/// Old clients that omit `failures` entirely (pre-this-PR wire format) still
/// work — `#[serde(default)]` on the field.
#[tokio::test]
async fn test_confirm_without_failures_field_is_backward_compatible() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    let queue_id: Uuid = sqlx::query_scalar(
        "INSERT INTO healthkit_write_queue (user_id, hk_type, value)
         VALUES ($1, 'body_mass', '{\"value\": 80.0, \"unit\": \"kg\"}'::jsonb)
         RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    // No `failures` key at all — the pre-PR wire shape.
    let body = json!({ "ids": [queue_id] });
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/confirm",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 204);

    let row: (Option<chrono::DateTime<chrono::Utc>>,) =
        sqlx::query_as("SELECT confirmed_at FROM healthkit_write_queue WHERE id = $1")
            .bind(queue_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert!(row.0.is_some());
}

// Note: the ADR-0008 cycle-guard regression (healthkit-sourced records never
// enqueue for write-back) is covered by
// `health_records::test_healthkit_sourced_record_never_enqueues_write_back`.
// `enqueue_write`'s only caller is `routes/health_records.rs`'s `create`
// handler — `POST /healthkit/sync` never calls it, so a test that posts to
// `/healthkit/sync` and asserts an empty write-queue proves nothing about the
// guard (it passes identically whether or not the guard exists).

/// User A cannot confirm or fail user B's write-queue rows — both `confirm`
/// and `mark_failed` are user-scoped.
#[tokio::test]
async fn test_confirm_and_failures_are_scoped_per_user() {
    let app = common::setup().await;
    let (user_a_id, _token_a) = common::create_test_user(&app).await;
    let (_user_b_id, token_b) = common::create_test_user(&app).await;

    let queue_id: Uuid = sqlx::query_scalar(
        "INSERT INTO healthkit_write_queue (user_id, hk_type, value)
         VALUES ($1, 'body_mass', '{\"value\": 80.0, \"unit\": \"kg\"}'::jsonb)
         RETURNING id",
    )
    .bind(user_a_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    // User B attempts to confirm user A's row.
    let confirm_body = json!({ "ids": [queue_id] });
    let confirm_response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/confirm",
            &token_b,
            Some(&confirm_body),
        ))
        .await
        .unwrap();
    assert_eq!(confirm_response.status(), 204);

    let row: (Option<chrono::DateTime<chrono::Utc>>,) =
        sqlx::query_as("SELECT confirmed_at FROM healthkit_write_queue WHERE id = $1")
            .bind(queue_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert!(
        row.0.is_none(),
        "user B must not be able to confirm user A's row"
    );

    // User B attempts to fail user A's row.
    let fail_body = json!({
        "ids": [],
        "failures": [{ "id": queue_id, "error": "cross-user attempt" }]
    });
    let fail_response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/confirm",
            &token_b,
            Some(&fail_body),
        ))
        .await
        .unwrap();
    assert_eq!(fail_response.status(), 204);

    let row: (Option<chrono::DateTime<chrono::Utc>>, Option<String>) =
        sqlx::query_as("SELECT failed_at, error FROM healthkit_write_queue WHERE id = $1")
            .bind(queue_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert!(
        row.0.is_none() && row.1.is_none(),
        "user B must not be able to mark user A's row failed"
    );
}

/// Unauthenticated requests to the write-queue endpoints are rejected with 401.
#[tokio::test]
async fn test_write_queue_endpoints_unauthenticated() {
    let app = common::setup().await;

    let get_response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/api/v1/healthkit/write-queue")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), 401);

    let confirm_response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/api/v1/healthkit/confirm")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&json!({ "ids": [] })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(confirm_response.status(), 401);
}

/// Malformed confirm bodies are rejected with a 4xx, never a 500.
#[tokio::test]
async fn test_confirm_invalid_body() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // `ids` must be an array of UUIDs, not strings.
    let body = json!({ "ids": ["not-a-uuid"] });
    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/healthkit/confirm",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    let status = response.status().as_u16();
    assert!(
        (400..500).contains(&status),
        "expected 4xx for malformed confirm body, got {status}"
    );
}
