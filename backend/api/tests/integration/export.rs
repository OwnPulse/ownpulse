// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;

use crate::common;

#[tokio::test]
async fn test_export_json() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Create some data so the export is not empty
    let hr_body = json!({
        "source": "manual",
        "record_type": "heart_rate",
        "value": 65.0,
        "unit": "bpm",
        "start_time": "2026-03-18T10:00:00Z"
    });
    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/health-records",
            &token,
            Some(&hr_body),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 201);

    // Export JSON
    let export_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/export/json",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(export_resp.status(), 200);

    let json = common::body_json(export_resp).await;
    assert_eq!(json["schema_version"], "0.1.0");
    assert!(json["health_records"].is_array());
    assert!(json["interventions"].is_array());
    assert!(json["daily_checkins"].is_array());
    assert!(json["lab_results"].is_array());
    assert!(json["observations"].is_array());
    assert!(
        !json["health_records"].as_array().unwrap().is_empty(),
        "export should contain the health record we created"
    );
}

#[tokio::test]
async fn test_export_csv() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Create a health record so CSV has data
    let hr_body = json!({
        "source": "manual",
        "record_type": "spo2",
        "value": 98.0,
        "unit": "%",
        "start_time": "2026-03-18T11:00:00Z"
    });
    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/health-records",
            &token,
            Some(&hr_body),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 201);

    // Export CSV
    let export_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/export/csv",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(export_resp.status(), 200);

    let csv_body = common::body_string(export_resp).await;
    assert!(
        csv_body.starts_with("id,source,record_type,value,unit,start_time,end_time"),
        "CSV should start with the expected header row, got: {}",
        csv_body.lines().next().unwrap_or("")
    );
    // Should have at least header + 1 data row
    assert!(
        csv_body.lines().count() >= 2,
        "CSV should have at least 2 lines"
    );
}
