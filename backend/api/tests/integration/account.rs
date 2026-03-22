// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::body::Body;
use http::Request;
use tower::ServiceExt;

use crate::common;

#[tokio::test]
async fn test_get_account() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .oneshot(common::auth_request("GET", "/api/v1/account", &token, None))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let json = common::body_json(response).await;
    assert!(json["email"].is_string());
    assert!(!json["email"].as_str().unwrap().is_empty());
    assert_eq!(json["auth_provider"], "local");
}

#[tokio::test]
async fn test_delete_account() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Delete account
    let delete_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            "/api/v1/account",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), 204);

    // Verify user is gone — GET account should fail
    // The JWT is still technically valid but the user no longer exists in the DB,
    // so the auth extractor or the handler will return an error.
    let get_resp = app
        .app
        .oneshot(common::auth_request("GET", "/api/v1/account", &token, None))
        .await
        .unwrap();
    // Could be 401 (extractor fails) or 404 (user not found) — either means gone
    assert!(
        get_resp.status() == 401 || get_resp.status() == 404,
        "expected 401 or 404 after account deletion, got {}",
        get_resp.status()
    );
}

#[tokio::test]
async fn test_unauthenticated_request() {
    let app = common::setup().await;

    // No Authorization header at all
    let response = app
        .app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/account")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}
