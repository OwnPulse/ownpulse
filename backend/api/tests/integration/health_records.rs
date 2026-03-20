// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

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
        "start_time": "2026-03-18T10:00:00Z"
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
        "start_time": "2026-03-18T08:00:00Z"
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
        "start_time": "2026-03-18T09:00:00Z"
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
        "start_time": "2026-03-18T07:00:00Z"
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
