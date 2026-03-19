// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;

use crate::common;

#[tokio::test]
async fn test_create_lab_result() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "panel_date": "2026-03-15",
        "marker": "testosterone_total",
        "value": 650.0,
        "unit": "ng/dL",
        "reference_low": 300.0,
        "reference_high": 1000.0,
        "source": "manual"
    });

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/labs",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 201);

    let json = common::body_json(response).await;
    assert_eq!(json["marker"], "testosterone_total");
    assert_eq!(json["value"], 650.0);
    assert_eq!(json["unit"], "ng/dL");
}

#[tokio::test]
async fn test_list_lab_results() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "panel_date": "2026-03-15",
        "marker": "creatinine",
        "value": 1.0,
        "unit": "mg/dL",
        "source": "manual"
    });

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/labs",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 201);

    let list_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/labs",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(list_resp.status(), 200);

    let json = common::body_json(list_resp).await;
    let items = json.as_array().expect("response should be an array");
    assert!(!items.is_empty());
    assert!(items.iter().any(|i| i["marker"] == "creatinine"));
}
