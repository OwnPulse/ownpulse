// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;

use crate::common;

#[tokio::test]
async fn test_create_observation() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "type": "event_instant",
        "name": "cold_plunge",
        "start_time": "2026-03-18T06:30:00Z",
        "value": {"notes": "3 minutes at 4C"},
        "source": "manual"
    });

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observations",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 201);

    let json = common::body_json(response).await;
    assert_eq!(json["type"], "event_instant");
    assert_eq!(json["name"], "cold_plunge");
}

#[tokio::test]
async fn test_invalid_observation_type() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "type": "invalid_type",
        "name": "something",
        "start_time": "2026-03-18T10:00:00Z",
        "source": "manual"
    });

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observations",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}
