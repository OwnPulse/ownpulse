// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;

use crate::common;

// ─── Invite CRUD ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_create_and_list_invites() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    // Create an invite
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/invites",
            &admin_token,
            Some(&json!({"label": "for friend", "max_uses": 5})),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 201);
    let body = common::body_json(response).await;
    assert_eq!(body["label"], "for friend");
    assert_eq!(body["max_uses"], 5);
    assert_eq!(body["use_count"], 0);
    assert!(body["code"].as_str().unwrap().len() == 16);
    let invite_id = body["id"].as_str().unwrap().to_string();

    // List invites
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/admin/invites",
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    let invites = body.as_array().unwrap();
    assert_eq!(invites.len(), 1);
    assert_eq!(invites[0]["id"].as_str().unwrap(), invite_id);
}

#[tokio::test]
async fn test_revoke_invite() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    // Create invite
    let response = app
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

    assert_eq!(response.status(), 201);
    let body = common::body_json(response).await;
    let invite_id = body["id"].as_str().unwrap().to_string();

    // Revoke it
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/admin/invites/{invite_id}"),
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    assert!(body["revoked_at"].is_string());
}

#[tokio::test]
async fn test_non_admin_cannot_create_invite() {
    let app = common::setup().await;
    let (_user_id, user_token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/invites",
            &user_token,
            Some(&json!({})),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 403);
}

// ─── Registration with invite ──────────────────────────────────────────────────

#[tokio::test]
async fn test_register_with_valid_invite() {
    let app = common::setup_with_config(|c| {
        c.require_invite = true;
    })
    .await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    // Create an invite code
    let response = app
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

    let invite_body = common::body_json(response).await;
    let code = invite_body["code"].as_str().unwrap();

    // Register with the invite code
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/auth/register",
            "",
            Some(&json!({
                "email": "newuser@example.com",
                "password": "securepassword123",
                "invite_code": code,
            })),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    assert!(body["access_token"].is_string());
}

#[tokio::test]
async fn test_register_without_invite_when_required() {
    let app = common::setup_with_config(|c| {
        c.require_invite = true;
    })
    .await;

    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&json!({
                        "email": "newuser@example.com",
                        "password": "securepassword123",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
    let body = common::body_json(response).await;
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("invite code required")
    );
}

#[tokio::test]
async fn test_register_with_invalid_invite() {
    let app = common::setup_with_config(|c| {
        c.require_invite = true;
    })
    .await;

    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&json!({
                        "email": "newuser@example.com",
                        "password": "securepassword123",
                        "invite_code": "NONEXISTENT12345",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
    let body = common::body_json(response).await;
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("invalid or expired")
    );
}

#[tokio::test]
async fn test_register_with_revoked_invite() {
    let app = common::setup_with_config(|c| {
        c.require_invite = true;
    })
    .await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    // Create and revoke an invite
    let response = app
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

    let invite_body = common::body_json(response).await;
    let code = invite_body["code"].as_str().unwrap().to_string();
    let invite_id = invite_body["id"].as_str().unwrap().to_string();

    // Revoke it
    let _revoke_response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/admin/invites/{invite_id}"),
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    // Try to register with the revoked code
    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&json!({
                        "email": "newuser@example.com",
                        "password": "securepassword123",
                        "invite_code": code,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_register_with_max_uses_exhausted() {
    let app = common::setup_with_config(|c| {
        c.require_invite = true;
    })
    .await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    // Create an invite with max_uses = 1
    let response = app
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

    let invite_body = common::body_json(response).await;
    let code = invite_body["code"].as_str().unwrap().to_string();

    // First registration should succeed
    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&json!({
                        "email": "first@example.com",
                        "password": "securepassword123",
                        "invite_code": code,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    // Second registration should fail
    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&json!({
                        "email": "second@example.com",
                        "password": "securepassword123",
                        "invite_code": code,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_register_without_invite_when_not_required() {
    // Default setup has require_invite = false
    let app = common::setup().await;

    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&json!({
                        "email": "openuser@example.com",
                        "password": "securepassword123",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    assert!(body["access_token"].is_string());
}

// ─── User status management ────────────────────────────────────────────────────

#[tokio::test]
async fn test_disable_user_blocks_access() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (user_id, user_token) = common::create_test_user(&app).await;

    // Verify user can access protected endpoint
    let response = app
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

    assert_eq!(response.status(), 200);

    // Admin disables user
    let response = app
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

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    assert_eq!(body["status"], "disabled");

    // Disabled user's request is now rejected
    let response = app
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

    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_reenable_user_restores_access() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (user_id, user_token) = common::create_test_user(&app).await;

    // Disable user
    let _response = app
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

    // Re-enable user
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/admin/users/{user_id}/status"),
            &admin_token,
            Some(&json!({"status": "active"})),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    // User can access again
    let response = app
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

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_admin_cannot_disable_self() {
    let app = common::setup().await;
    let (admin_id, admin_token) = common::create_admin_user(&app).await;

    let response = app
        .app
        .clone()
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

#[tokio::test]
async fn test_admin_cannot_delete_self() {
    let app = common::setup().await;
    let (admin_id, admin_token) = common::create_admin_user(&app).await;

    let response = app
        .app
        .clone()
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
async fn test_admin_delete_user() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (user_id, _user_token) = common::create_test_user(&app).await;

    // Must disable the user before deleting
    let response = app
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
    assert_eq!(response.status(), 200);

    let response = app
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

    assert_eq!(response.status(), 204);

    // Verify user is gone from the list
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/admin/users",
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    let body = common::body_json(response).await;
    let users = body.as_array().unwrap();
    assert!(
        !users
            .iter()
            .any(|u| u["id"].as_str() == Some(&user_id.to_string())),
        "deleted user should not appear in user list"
    );
}

#[tokio::test]
async fn test_register_short_password_rejected() {
    let app = common::setup().await;

    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&json!({
                        "email": "short@example.com",
                        "password": "short",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_register_invalid_email_rejected() {
    let app = common::setup().await;

    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&json!({
                        "email": "notanemail",
                        "password": "securepassword123",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_list_users_includes_status() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/admin/users",
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    let users = body.as_array().unwrap();
    for user in users {
        assert!(
            user["status"].is_string(),
            "every user should have a status field"
        );
    }
}

#[tokio::test]
async fn test_update_role_still_works() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (user_id, _user_token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/admin/users/{user_id}/role"),
            &admin_token,
            Some(&json!({"role": "admin"})),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    assert_eq!(body["role"], "admin");
}
