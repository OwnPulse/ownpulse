// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;

use crate::common;

#[tokio::test]
async fn test_create_intervention() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "substance": "caffeine",
        "dose": 200.0,
        "unit": "mg",
        "route": "oral",
        "administered_at": "2026-03-18T07:30:00Z"
    });

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/interventions",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 201);

    let json = common::body_json(response).await;
    assert_eq!(json["substance"], "caffeine");
    assert_eq!(json["dose"], 200.0);
    assert_eq!(json["unit"], "mg");
}

#[tokio::test]
async fn test_list_interventions() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "substance": "magnesium",
        "dose": 400.0,
        "unit": "mg",
        "administered_at": "2026-03-18T21:00:00Z"
    });

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/interventions",
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
            "/api/v1/interventions",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(list_resp.status(), 200);

    let json = common::body_json(list_resp).await;
    let items = json.as_array().expect("response should be an array");
    assert!(!items.is_empty());
    assert!(items.iter().any(|i| i["substance"] == "magnesium"));
}

#[tokio::test]
async fn test_delete_intervention() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "substance": "vitamin_d",
        "dose": 5000.0,
        "unit": "IU",
        "administered_at": "2026-03-18T08:00:00Z"
    });

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/interventions",
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
            &format!("/api/v1/interventions/{id}"),
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
            &format!("/api/v1/interventions/{id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 404);
}
