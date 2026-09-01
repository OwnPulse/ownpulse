// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use api::models::health_record::CreateHealthRecord;
use serde_json::json;
use tower::ServiceExt;

use crate::common;

#[tokio::test]
async fn test_create_health_record() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "source": "manual",
        "record_type": "heart_rate",
        "value": 72.0,
        "unit": "bpm",
        "start_time": "2026-03-18T10:00:00Z" // date-ok
    });

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/health-records",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 201);

    let json = common::body_json(response).await;
    assert_eq!(json["record_type"], "heart_rate");
    assert_eq!(json["value"], 72.0);
    assert_eq!(json["unit"], "bpm");
}

#[tokio::test]
async fn test_list_health_records() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "source": "manual",
        "record_type": "weight",
        "value": 80.5,
        "unit": "kg",
        "start_time": "2026-03-18T08:00:00Z" // date-ok
    });

    // Create a record
    let create_resp = app
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
    assert_eq!(create_resp.status(), 201);

    // List records
    let list_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/health-records",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(list_resp.status(), 200);

    let json = common::body_json(list_resp).await;
    let records = json.as_array().expect("response should be an array");
    assert!(!records.is_empty());
    assert!(records.iter().any(|r| r["record_type"] == "weight"));
}

#[tokio::test]
async fn test_get_health_record() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "source": "manual",
        "record_type": "blood_pressure_systolic",
        "value": 120.0,
        "unit": "mmHg",
        "start_time": "2026-03-18T09:00:00Z" // date-ok
    });

    let create_resp = app
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
    assert_eq!(create_resp.status(), 201);
    let created = common::body_json(create_resp).await;
    let id = created["id"].as_str().unwrap();

    // Get by id
    let get_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/health-records/{id}"),
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(get_resp.status(), 200);
    let fetched = common::body_json(get_resp).await;
    assert_eq!(fetched["id"], id);
    assert_eq!(fetched["record_type"], "blood_pressure_systolic");
}

#[tokio::test]
async fn test_delete_health_record() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "source": "manual",
        "record_type": "temperature",
        "value": 36.6,
        "unit": "celsius",
        "start_time": "2026-03-18T07:00:00Z" // date-ok
    });

    let create_resp = app
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
    assert_eq!(create_resp.status(), 201);
    let created = common::body_json(create_resp).await;
    let id = created["id"].as_str().unwrap();

    // Delete
    let delete_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/health-records/{id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), 204);

    // Verify gone
    let get_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/health-records/{id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 404);
}

/// Regression test: deleting a record by id that belongs to another user
/// must (1) return 404 (ownership is verified, not just "any row with this
/// id") and (2) must NOT clear that other user's `duplicate_of` provenance
/// link. Previously the `duplicate_of` cleanup ran before ownership was
/// checked and had no `user_id` filter, so an attacker who knew (or guessed)
/// another user's record id could sever their dedup provenance without ever
/// being able to read or own that record.
#[tokio::test]
async fn test_delete_does_not_clear_other_users_duplicate_of() {
    let app = common::setup().await;
    let (_user_a_id, token_a) = common::create_test_user(&app).await;
    let (user_b_id, _token_b) = common::create_test_user(&app).await;

    // User B has two records: an original and a record that was detected as
    // a cross-source duplicate of it (duplicate_of = original.id).
    let original = api::db::health_records::insert(
        &app.pool,
        user_b_id,
        &CreateHealthRecord {
            source: "garmin".to_string(),
            record_type: "heart_rate".to_string(),
            value: Some(70.0),
            unit: Some("bpm".to_string()),
            start_time: "2026-03-18T14:00:00Z".parse().unwrap(), // date-ok
            end_time: None,
            metadata: None,
            source_id: None,
        },
        None,
    )
    .await
    .unwrap();

    let duplicate = api::db::health_records::insert(
        &app.pool,
        user_b_id,
        &CreateHealthRecord {
            source: "oura".to_string(),
            record_type: "heart_rate".to_string(),
            value: Some(70.2),
            unit: Some("bpm".to_string()),
            start_time: "2026-03-18T14:00:05Z".parse().unwrap(), // date-ok
            end_time: None,
            metadata: None,
            source_id: None,
        },
        Some(original.id),
    )
    .await
    .unwrap();
    assert_eq!(duplicate.duplicate_of, Some(original.id));

    // User A attempts to delete user B's original record by id.
    let delete_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/health-records/{}", original.id),
            &token_a,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), 404);

    // User B's original record is untouched.
    let still_exists: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM health_records WHERE id = $1 AND user_id = $2")
            .bind(original.id)
            .bind(user_b_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(still_exists.0, 1, "user B's record must not be deleted");

    // User B's duplicate_of provenance link must not have been cleared.
    let duplicate_of: (Option<uuid::Uuid>,) =
        sqlx::query_as("SELECT duplicate_of FROM health_records WHERE id = $1")
            .bind(duplicate.id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(
        duplicate_of.0,
        Some(original.id),
        "user A must not be able to clear user B's duplicate_of provenance link"
    );
}

/// Positive counterpart: deleting a record the caller actually owns still
/// clears `duplicate_of` links on their own referring records. This is the
/// regression guard for the cleanup query itself — without the `user_id`
/// scoping (or with the whole `UPDATE` removed), this still passes, so it's
/// `test_delete_does_not_clear_other_users_duplicate_of` above that catches
/// cross-tenant scope regressions and this one that catches the cleanup
/// being deleted or broken outright.
#[tokio::test]
async fn test_delete_clears_own_duplicate_of() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    let original = api::db::health_records::insert(
        &app.pool,
        user_id,
        &CreateHealthRecord {
            source: "garmin".to_string(),
            record_type: "heart_rate".to_string(),
            value: Some(70.0),
            unit: Some("bpm".to_string()),
            start_time: "2026-03-18T15:00:00Z".parse().unwrap(), // date-ok
            end_time: None,
            metadata: None,
            source_id: None,
        },
        None,
    )
    .await
    .unwrap();

    let duplicate = api::db::health_records::insert(
        &app.pool,
        user_id,
        &CreateHealthRecord {
            source: "oura".to_string(),
            record_type: "heart_rate".to_string(),
            value: Some(70.2),
            unit: Some("bpm".to_string()),
            start_time: "2026-03-18T15:00:05Z".parse().unwrap(), // date-ok
            end_time: None,
            metadata: None,
            source_id: None,
        },
        Some(original.id),
    )
    .await
    .unwrap();
    assert_eq!(duplicate.duplicate_of, Some(original.id));

    let delete_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/health-records/{}", original.id),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), 204);

    let duplicate_of: (Option<uuid::Uuid>,) =
        sqlx::query_as("SELECT duplicate_of FROM health_records WHERE id = $1")
            .bind(duplicate.id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(
        duplicate_of.0, None,
        "deleting the referenced record must clear the caller's own duplicate_of link"
    );
}

/// Cycle guard (ADR-0008): a record with `source = "healthkit"` must NOT
/// create a `healthkit_write_queue` row. Writing a HealthKit-sourced record
/// back to HealthKit would create an infinite read→write→read cycle. The guard
/// is enforced unconditionally in the service layer (`db::healthkit::enqueue_write`),
/// not in the route handler, so it cannot be bypassed by any API parameter.
#[tokio::test]
async fn test_healthkit_sourced_record_never_enqueues_write_back() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "source": "healthkit",
        "record_type": "heart_rate",
        "value": 72.0,
        "unit": "bpm",
        "start_time": "2026-03-18T10:00:00Z" // date-ok
    });

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/health-records",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 201);

    // No write-back row may exist for a healthkit-sourced record.
    let queue_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM healthkit_write_queue WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(
        queue_count.0, 0,
        "healthkit-sourced record must never be enqueued for HealthKit write-back"
    );
}

/// Counterpart to the cycle-guard test: a non-HealthKit source (manual) with a
/// HealthKit mapping DOES create a `healthkit_write_queue` row, so write-back
/// works for legitimately user-entered data. Proves the guard is specific to
/// `source = "healthkit"` and does not over-filter.
#[tokio::test]
async fn test_manual_record_enqueues_write_back() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "source": "manual",
        "record_type": "heart_rate",
        "value": 72.0,
        "unit": "bpm",
        "start_time": "2026-03-18T11:00:00Z" // date-ok
    });

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/health-records",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 201);

    // Exactly one write-back row, referencing the source record.
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT hk_type, source_table FROM healthkit_write_queue WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_all(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "manual record with a HealthKit mapping must be enqueued for write-back"
    );
    assert_eq!(rows[0].0, "heart_rate");
    assert_eq!(rows[0].1.as_deref(), Some("health_records"));
}

/// Belt-and-braces: even if a caller attempts to spoof `source = "healthkit"`
/// through the public POST endpoint, the inserted record is stored verbatim
/// with `source = "healthkit"` (POST /health-records does not rewrite source),
/// and the cycle guard still prevents any write-back row. This proves the
/// guard keys on the persisted record source and cannot be bypassed via the
/// request body.
#[tokio::test]
async fn test_cycle_guard_not_bypassable_via_request_body() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    // Two records in the same request window: one healthkit, one manual.
    for (source, start_time) in [
        ("healthkit", "2026-03-18T12:00:00Z"), // date-ok
        ("manual", "2026-03-18T13:00:00Z"),    // date-ok
    ] {
        let body = json!({
            "source": source,
            "record_type": "steps",
            "value": 1000.0,
            "unit": "count",
            "start_time": start_time
        });
        let response = app
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
        assert_eq!(response.status(), 201);
    }

    // Exactly one write-back row — only the manual record, never the healthkit one.
    let rows: Vec<(Option<String>,)> = sqlx::query_as(
        "SELECT q.source_table
         FROM healthkit_write_queue q
         WHERE q.user_id = $1",
    )
    .bind(user_id)
    .fetch_all(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "only the non-healthkit record may be enqueued; the healthkit record must be skipped"
    );

    // And confirm the write-queue entry's source record is the manual one.
    let manual_enqueued: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM healthkit_write_queue q
         JOIN health_records r ON r.id = q.source_record_id
         WHERE q.user_id = $1 AND r.source = 'manual'",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(manual_enqueued.0, 1);
}
