// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;

use crate::common;

#[tokio::test]
async fn test_create_sleep_record() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "date": "2026-03-18",
        "sleep_start": "2026-03-17T23:00:00Z",
        "sleep_end": "2026-03-18T07:00:00Z",
        "duration_minutes": 480,
        "deep_minutes": 90,
        "light_minutes": 240,
        "rem_minutes": 120,
        "awake_minutes": 30,
        "score": 82,
        "source": "manual"
    });

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/sleep",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 201);

    let json = common::body_json(response).await;
    assert_eq!(json["date"], "2026-03-18");
    assert_eq!(json["duration_minutes"], 480);
    assert_eq!(json["score"], 82);
    assert_eq!(json["source"], "manual");
}

#[tokio::test]
async fn test_create_sleep_record_defaults_source_to_manual() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "date": "2026-03-19",
        "duration_minutes": 420
    });

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/sleep",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 201);

    let json = common::body_json(response).await;
    assert_eq!(json["source"], "manual");
}

#[tokio::test]
async fn test_list_sleep_records() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "date": "2026-03-18",
        "duration_minutes": 480,
        "source": "oura"
    });

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/sleep",
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
            "/api/v1/sleep",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(list_resp.status(), 200);

    let json = common::body_json(list_resp).await;
    let items = json.as_array().expect("response should be an array");
    assert!(!items.is_empty());
    assert!(items.iter().any(|r| r["date"] == "2026-03-18"));
}

#[tokio::test]
async fn test_list_sleep_records_with_date_filter() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Create two records on different dates
    for (date, source) in [("2026-03-10", "manual"), ("2026-03-20", "garmin")] {
        let body = json!({
            "date": date,
            "duration_minutes": 450,
            "source": source
        });
        let resp = app
            .app
            .clone()
            .oneshot(common::auth_request(
                "POST",
                "/api/v1/sleep",
                &token,
                Some(&body),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), 201);
    }

    // Filter to only the earlier date
    let list_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/sleep?start=2026-03-10&end=2026-03-15",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(list_resp.status(), 200);
    let json = common::body_json(list_resp).await;
    let items = json.as_array().expect("response should be an array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["date"], "2026-03-10");
}

#[tokio::test]
async fn test_get_sleep_record_by_id() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "date": "2026-03-18",
        "duration_minutes": 390,
        "score": 75
    });

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/sleep",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 201);
    let created = common::body_json(create_resp).await;
    let id = created["id"].as_str().unwrap();

    let get_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/sleep/{id}"),
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(get_resp.status(), 200);
    let json = common::body_json(get_resp).await;
    assert_eq!(json["id"], id);
    assert_eq!(json["score"], 75);
}

#[tokio::test]
async fn test_delete_sleep_record() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "date": "2026-03-18",
        "duration_minutes": 420
    });

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/sleep",
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
            &format!("/api/v1/sleep/{id}"),
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
            &format!("/api/v1/sleep/{id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 404);
}

#[tokio::test]
async fn test_sleep_requires_auth() {
    let app = common::setup().await;

    // POST without token
    let body = json!({
        "date": "2026-03-18",
        "duration_minutes": 480
    });

    use axum::body::Body;
    use http::Request;

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/sleep")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let response = app.app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 401);
}
