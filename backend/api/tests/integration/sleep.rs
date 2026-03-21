// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;

use crate::common;

#[tokio::test]
async fn test_create_sleep_record() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let today = chrono::Utc::now().date_naive();
    let sleep_end = today.and_hms_opt(7, 0, 0).unwrap();
    let sleep_start = (today - chrono::Duration::days(1))
        .and_hms_opt(23, 0, 0)
        .unwrap();

    let body = json!({
        "date": today.to_string(),
        "sleep_start": sleep_start.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        "sleep_end": sleep_end.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
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
    assert_eq!(json["date"], today.to_string());
    assert_eq!(json["duration_minutes"], 480);
    assert_eq!(json["score"], 82);
    assert_eq!(json["source"], "manual");
}

#[tokio::test]
async fn test_create_sleep_record_defaults_source_to_manual() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let today = chrono::Utc::now().date_naive();

    let body = json!({
        "date": today.to_string(),
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

    let today = chrono::Utc::now().date_naive();

    let body = json!({
        "date": today.to_string(),
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
    assert!(items.iter().any(|r| r["date"] == today.to_string()));
}

#[tokio::test]
async fn test_list_sleep_records_with_date_filter() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let today = chrono::Utc::now().date_naive();
    let ten_days_ago = today - chrono::Duration::days(10);

    for (date, source) in [
        (ten_days_ago.to_string(), "manual"),
        (today.to_string(), "garmin"),
    ] {
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

    // Filter window: ten_days_ago to (today - 5 days), which excludes "today".
    let filter_end = today - chrono::Duration::days(5);
    let list_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            &format!(
                "/api/v1/sleep?start={}&end={}",
                ten_days_ago, filter_end
            ),
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(list_resp.status(), 200);
    let json = common::body_json(list_resp).await;
    let items = json.as_array().expect("response should be an array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["date"], ten_days_ago.to_string());
}

#[tokio::test]
async fn test_get_sleep_record_by_id() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let today = chrono::Utc::now().date_naive();

    let body = json!({
        "date": today.to_string(),
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

    let today = chrono::Utc::now().date_naive();

    let body = json!({
        "date": today.to_string(),
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
async fn test_delete_nonexistent_sleep_record_returns_404() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let random_id = uuid::Uuid::new_v4();

    let delete_resp = app
        .app
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/sleep/{random_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(delete_resp.status(), 404);
}

#[tokio::test]
async fn test_cross_user_isolation() {
    let app = common::setup().await;
    let (_user_a_id, token_a) = common::create_test_user(&app).await;
    let (_user_b_id, token_b) = common::create_test_user(&app).await;

    let today = chrono::Utc::now().date_naive();

    // User A creates a sleep record.
    let body = json!({
        "date": today.to_string(),
        "duration_minutes": 450
    });

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/sleep",
            &token_a,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 201);
    let created = common::body_json(create_resp).await;
    let id = created["id"].as_str().unwrap();

    // User B tries to GET the record — must be 404.
    let get_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/sleep/{id}"),
            &token_b,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 404);

    // User B tries to DELETE the record — must be 404.
    let delete_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/sleep/{id}"),
            &token_b,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), 404);

    // Confirm the record still exists for user A.
    let get_after_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/sleep/{id}"),
            &token_a,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(
        get_after_resp.status(),
        200,
        "user A's record must still exist after user B's delete attempt"
    );
}

#[tokio::test]
async fn test_sleep_requires_auth() {
    let app = common::setup().await;

    let today = chrono::Utc::now().date_naive();

    let body = json!({
        "date": today.to_string(),
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

#[tokio::test]
async fn test_sleep_stored_as_observation() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let today = chrono::Utc::now().date_naive();

    let body = json!({
        "date": today.to_string(),
        "duration_minutes": 480,
        "score": 85,
        "source": "manual"
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

    // Verify the record is also visible via the observations endpoint.
    let obs_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/observations?type=sleep",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(obs_resp.status(), 200);

    let obs_json = common::body_json(obs_resp).await;
    let items = obs_json.as_array().expect("response should be an array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["type"], "sleep");
    assert_eq!(items[0]["value"]["duration_minutes"], 480);
    assert_eq!(items[0]["value"]["score"], 85);
}
