// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;

use crate::common;

#[tokio::test]
async fn test_upsert_checkin() {
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
async fn test_upsert_checkin_updates() {
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

    // Second upsert for same date — should update
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
    let updated = common::body_json(resp2).await;
    assert_eq!(updated["energy"], 9);
    assert_eq!(updated["mood"], 10);

    // List should show only one checkin for this date
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
    assert_eq!(matching.len(), 1);
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
