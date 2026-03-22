// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::body::Body;
use http::Request;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::common;

/// Helper: collect the response body into a parsed JSON value.
async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Helper: build a POST request with JSON body.
fn post_json(uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .unwrap()
}

// ── Invite CRUD ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_create_invite_as_admin() {
    let app = common::setup().await;
    let (_admin_id, token) = common::create_admin_user(&app).await;

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/invites",
            &token,
            Some(&json!({"label": "for bob", "max_uses": 5})),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 201);

    let json = body_json(response).await;
    assert!(json["code"].is_string());
    assert_eq!(json["code"].as_str().unwrap().len(), 16);
    assert_eq!(json["label"], "for bob");
    assert_eq!(json["max_uses"], 5);
    assert_eq!(json["use_count"], 0);
    assert!(json["revoked_at"].is_null());
}

#[tokio::test]
async fn test_list_invites_as_admin() {
    let app = common::setup().await;
    let (_admin_id, token) = common::create_admin_user(&app).await;

    // Create two invites
    app.app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/invites",
            &token,
            Some(&json!({"label": "invite-1"})),
        ))
        .await
        .unwrap();

    app.app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/invites",
            &token,
            Some(&json!({"label": "invite-2"})),
        ))
        .await
        .unwrap();

    let response = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/admin/invites",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let json = body_json(response).await;
    let invites = json.as_array().unwrap();
    assert_eq!(invites.len(), 2);
}

#[tokio::test]
async fn test_revoke_invite() {
    let app = common::setup().await;
    let (_admin_id, token) = common::create_admin_user(&app).await;

    // Create an invite
    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/invites",
            &token,
            Some(&json!({})),
        ))
        .await
        .unwrap();

    let invite = body_json(create_resp).await;
    let invite_id = invite["id"].as_str().unwrap();

    // Revoke it
    let revoke_resp = app
        .app
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/admin/invites/{invite_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(revoke_resp.status(), 200);
    let revoked = body_json(revoke_resp).await;
    assert!(!revoked["revoked_at"].is_null());
}

#[tokio::test]
async fn test_non_admin_cannot_create_invites() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/invites",
            &token,
            Some(&json!({})),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 403);
}

// ── Registration with invite codes ───────────────────────────────────

#[tokio::test]
async fn test_register_with_valid_invite() {
    let app = common::setup_with_invites().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    // Create an invite
    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/invites",
            &admin_token,
            Some(&json!({})),
        ))
        .await
        .unwrap();

    let invite = body_json(create_resp).await;
    let code = invite["code"].as_str().unwrap();

    // Register with the invite code
    let register_resp = app
        .app
        .oneshot(post_json(
            "/api/v1/auth/register",
            &json!({
                "email": "newuser@example.com",
                "password": "securepassword123",
                "invite_code": code
            }),
        ))
        .await
        .unwrap();

    assert_eq!(register_resp.status(), 200);
    let json = body_json(register_resp).await;
    assert!(json["access_token"].is_string());
}

#[tokio::test]
async fn test_register_without_invite_when_required() {
    let app = common::setup_with_invites().await;

    let response = app
        .app
        .oneshot(post_json(
            "/api/v1/auth/register",
            &json!({
                "email": "newuser@example.com",
                "password": "securepassword123"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_register_with_invalid_invite() {
    let app = common::setup_with_invites().await;

    let response = app
        .app
        .oneshot(post_json(
            "/api/v1/auth/register",
            &json!({
                "email": "newuser@example.com",
                "password": "securepassword123",
                "invite_code": "nonexistent12345"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_register_with_revoked_invite() {
    let app = common::setup_with_invites().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    // Create and revoke an invite
    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/invites",
            &admin_token,
            Some(&json!({})),
        ))
        .await
        .unwrap();

    let invite = body_json(create_resp).await;
    let invite_id = invite["id"].as_str().unwrap();
    let code = invite["code"].as_str().unwrap().to_string();

    app.app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/admin/invites/{invite_id}"),
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    // Try to register with revoked code
    let response = app
        .app
        .oneshot(post_json(
            "/api/v1/auth/register",
            &json!({
                "email": "newuser@example.com",
                "password": "securepassword123",
                "invite_code": code
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_register_with_exhausted_invite() {
    let app = common::setup_with_invites().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    // Create an invite with max_uses=1
    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/invites",
            &admin_token,
            Some(&json!({"max_uses": 1})),
        ))
        .await
        .unwrap();

    let invite = body_json(create_resp).await;
    let code = invite["code"].as_str().unwrap().to_string();

    // First registration should succeed
    let resp1 = app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/register",
            &json!({
                "email": "first@example.com",
                "password": "securepassword123",
                "invite_code": code
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp1.status(), 200);

    // Second registration with same code should fail
    let resp2 = app
        .app
        .oneshot(post_json(
            "/api/v1/auth/register",
            &json!({
                "email": "second@example.com",
                "password": "securepassword123",
                "invite_code": code
            }),
        ))
        .await
        .unwrap();

    assert_eq!(resp2.status(), 400);
}

#[tokio::test]
async fn test_register_without_invite_when_not_required() {
    // Default setup has require_invite=false
    let app = common::setup().await;

    let response = app
        .app
        .oneshot(post_json(
            "/api/v1/auth/register",
            &json!({
                "email": "openuser@example.com",
                "password": "securepassword123"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let json = body_json(response).await;
    assert!(json["access_token"].is_string());
}

// ── User status management ───────────────────────────────────────────

#[tokio::test]
async fn test_disable_user_blocks_access() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (user_id, user_token) = common::create_test_user(&app).await;

    // Verify user can access their account
    let get_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/account",
            &user_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 200);

    // Disable the user
    let disable_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/admin/users/{user_id}/status"),
            &admin_token,
            Some(&json!({"status": "disabled"})),
        ))
        .await
        .unwrap();
    assert_eq!(disable_resp.status(), 200);

    let json = body_json(disable_resp).await;
    assert_eq!(json["status"], "disabled");

    // Disabled user should be rejected
    let blocked_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/account",
            &user_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(blocked_resp.status(), 403);
}

#[tokio::test]
async fn test_reenable_user_restores_access() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (user_id, user_token) = common::create_test_user(&app).await;

    // Disable the user
    app.app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/admin/users/{user_id}/status"),
            &admin_token,
            Some(&json!({"status": "disabled"})),
        ))
        .await
        .unwrap();

    // Re-enable
    app.app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/admin/users/{user_id}/status"),
            &admin_token,
            Some(&json!({"status": "active"})),
        ))
        .await
        .unwrap();

    // User should be able to access again
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/account",
            &user_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_admin_cannot_disable_self() {
    let app = common::setup().await;
    let (admin_id, admin_token) = common::create_admin_user(&app).await;

    let response = app
        .app
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/admin/users/{admin_id}/status"),
            &admin_token,
            Some(&json!({"status": "disabled"})),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

// ── Admin user delete ────────────────────────────────────────────────

#[tokio::test]
async fn test_admin_delete_user() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (user_id, user_token) = common::create_test_user(&app).await;

    let delete_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/admin/users/{user_id}"),
            &admin_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), 204);

    // Deleted user should not be able to access anything
    let get_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/account",
            &user_token,
            None,
        ))
        .await
        .unwrap();
    assert!(
        get_resp.status() == 401 || get_resp.status() == 404,
        "expected 401 or 404 after deletion, got {}",
        get_resp.status()
    );
}

#[tokio::test]
async fn test_admin_cannot_delete_self() {
    let app = common::setup().await;
    let (admin_id, admin_token) = common::create_admin_user(&app).await;

    let response = app
        .app
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/admin/users/{admin_id}"),
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_user_list_includes_status() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    common::create_test_user(&app).await;

    let response = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/admin/users",
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let json = body_json(response).await;
    let users = json.as_array().unwrap();
    assert!(users.len() >= 2);
    for user in users {
        assert!(user["status"].is_string());
        assert_eq!(user["status"], "active");
    }
}
