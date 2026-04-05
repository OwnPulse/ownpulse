// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;

use crate::common;

#[tokio::test]
async fn test_create_checkin() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "date": "2026-03-18",
        "energy": 7,
        "mood": 8,
        "focus": 6,
        "recovery": 5,
        "libido": 4
    });

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/checkins",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 201);

    let json = common::body_json(response).await;
    assert_eq!(json["date"], "2026-03-18");
    assert_eq!(json["energy"], 7);
    assert_eq!(json["mood"], 8);
}

#[tokio::test]
async fn test_multiple_checkins_same_day() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body1 = json!({
        "date": "2026-03-18",
        "energy": 5,
        "mood": 6
    });

    let resp1 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/checkins",
            &token,
            Some(&body1),
        ))
        .await
        .unwrap();
    assert_eq!(resp1.status(), 201);

    // Second POST for same date — should create a second checkin
    let body2 = json!({
        "date": "2026-03-18",
        "energy": 9,
        "mood": 10
    });

    let resp2 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/checkins",
            &token,
            Some(&body2),
        ))
        .await
        .unwrap();
    assert_eq!(resp2.status(), 201);

    // List should show two checkins for this date
    let list_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/checkins",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(list_resp.status(), 200);
    let items = common::body_json(list_resp).await;
    let checkins = items.as_array().unwrap();
    let matching: Vec<_> = checkins
        .iter()
        .filter(|c| c["date"] == "2026-03-18")
        .collect();
    assert_eq!(matching.len(), 2);
}

#[tokio::test]
async fn test_update_checkin() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Create a checkin
    let body = json!({
        "date": "2026-03-18",
        "energy": 5,
        "mood": 6,
        "focus": 4
    });

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/checkins",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 201);
    let created = common::body_json(create_resp).await;
    let checkin_id = created["id"].as_str().unwrap();

    // Update the checkin via PUT
    let update_body = json!({
        "date": "2026-03-18",
        "energy": 9,
        "mood": 10,
        "focus": 8
    });

    let update_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/checkins/{checkin_id}"),
            &token,
            Some(&update_body),
        ))
        .await
        .unwrap();
    assert_eq!(update_resp.status(), 200);
    let updated = common::body_json(update_resp).await;
    assert_eq!(updated["energy"], 9);
    assert_eq!(updated["mood"], 10);
    assert_eq!(updated["focus"], 8);

    // GET to verify the update persisted
    let get_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/checkins/{checkin_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 200);
    let fetched = common::body_json(get_resp).await;
    assert_eq!(fetched["energy"], 9);
    assert_eq!(fetched["mood"], 10);
    assert_eq!(fetched["focus"], 8);
}

#[tokio::test]
async fn test_checkin_validates_score_range() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "date": "2026-03-18",
        "energy": 11
    });

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/checkins",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_update_nonexistent_checkin_returns_404() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "date": "2026-03-18",
        "energy": 5
    });

    let response = app
        .app
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/checkins/{}", uuid::Uuid::new_v4()),
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    // fetch_one on no matching row → RowNotFound → 404
    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_update_other_users_checkin_returns_404() {
    let app = common::setup().await;
    let (_user1_id, token1) = common::create_test_user(&app).await;
    let (_user2_id, token2) = common::create_test_user(&app).await;

    // User 1 creates a checkin
    let body = json!({
        "date": "2026-03-18",
        "energy": 5,
        "mood": 6
    });

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/checkins",
            &token1,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 201);
    let created = common::body_json(create_resp).await;
    let checkin_id = created["id"].as_str().unwrap();

    // User 2 tries to update user 1's checkin
    let update_body = json!({
        "date": "2026-03-18",
        "energy": 10
    });

    let update_resp = app
        .app
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/checkins/{checkin_id}"),
            &token2,
            Some(&update_body),
        ))
        .await
        .unwrap();

    // user_id scoping means this looks like "not found"
    assert_eq!(update_resp.status(), 404);
}
