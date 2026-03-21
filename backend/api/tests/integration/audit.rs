// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;

use crate::common;

/// After a JSON export the audit log should contain one "export" entry.
#[tokio::test]
async fn test_export_creates_audit_entry() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Trigger a JSON export.
    let export_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/export/json",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(export_resp.status(), 200);

    // The tokio::spawn inside the handler is fire-and-forget. Give it a moment
    // to complete before querying the audit log.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Audit log should have the entry.
    let log_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/account/audit-log",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(log_resp.status(), 200);

    let entries = common::body_json(log_resp).await;
    let arr = entries.as_array().expect("audit-log should be an array");
    assert!(
        arr.iter().any(|e| e["action"] == "export" && e["resource_type"] == "json"),
        "expected an export/json audit entry, got: {arr:?}"
    );
}

/// After a CSV export the audit log should contain one "export/csv" entry.
#[tokio::test]
async fn test_csv_export_creates_audit_entry() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let export_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/export/csv",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(export_resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let log_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/account/audit-log",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(log_resp.status(), 200);

    let entries = common::body_json(log_resp).await;
    let arr = entries.as_array().expect("audit-log should be an array");
    assert!(
        arr.iter().any(|e| e["action"] == "export" && e["resource_type"] == "csv"),
        "expected an export/csv audit entry, got: {arr:?}"
    );
}

/// Deleting a health record should produce a "delete/health_record" audit entry
/// that carries the deleted record's id.
#[tokio::test]
async fn test_health_record_delete_creates_audit_entry() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Create a record.
    let create_body = json!({
        "source": "manual",
        "record_type": "heart_rate",
        "value": 72.0,
        "unit": "bpm",
        "start_time": "2026-03-21T10:00:00Z"
    });
    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/health-records",
            &token,
            Some(&create_body),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 201);
    let created = common::body_json(create_resp).await;
    let record_id = created["id"].as_str().unwrap().to_owned();

    // Delete it.
    let delete_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/health-records/{record_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), 204);

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Audit log should reflect the delete.
    let log_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/account/audit-log",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(log_resp.status(), 200);

    let entries = common::body_json(log_resp).await;
    let arr = entries.as_array().expect("audit-log should be an array");
    assert!(
        arr.iter().any(|e| {
            e["action"] == "delete"
                && e["resource_type"] == "health_record"
                && e["resource_id"] == record_id
        }),
        "expected a delete/health_record audit entry with id={record_id}, got: {arr:?}"
    );
}

/// The audit log endpoint requires authentication.
#[tokio::test]
async fn test_audit_log_requires_auth() {
    use axum::body::Body;
    use http::Request;

    let app = common::setup().await;

    let response = app
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/account/audit-log")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

/// One user cannot see another user's audit log entries.
#[tokio::test]
async fn test_audit_log_is_scoped_to_user() {
    let app = common::setup().await;
    let (_user_a, token_a) = common::create_test_user(&app).await;
    let (_user_b, token_b) = common::create_test_user(&app).await;

    // User A performs an export.
    let export_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/export/json",
            &token_a,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(export_resp.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // User B's audit log should be empty.
    let log_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/account/audit-log",
            &token_b,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(log_resp.status(), 200);

    let entries = common::body_json(log_resp).await;
    let arr = entries.as_array().expect("audit-log should be an array");
    assert!(
        arr.is_empty(),
        "user B should not see user A's audit entries, got: {arr:?}"
    );
}
