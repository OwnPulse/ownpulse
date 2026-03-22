// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

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

    // Disable the user first — deletion requires disabled status.
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

// ─── Expired invite code ────────────────────────────────────────────────────

#[tokio::test]
async fn test_register_with_expired_invite() {
    let app = common::setup_with_config(|c| {
        c.require_invite = true;
    })
    .await;
    let (admin_id, _admin_token) = common::create_admin_user(&app).await;

    // Insert an invite directly with expires_at in the past
    let code = "EXPIRED123456789";
    sqlx::query(
        "INSERT INTO invite_codes (code, created_by, expires_at)
         VALUES ($1, $2, now() - interval '1 day')",
    )
    .bind(code)
    .bind(admin_id)
    .execute(&app.pool)
    .await
    .expect("failed to insert expired invite");

    // Try to register with the expired code
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
                        "email": "expired-invite-user@example.com",
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
    let body = common::body_json(response).await;
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("invalid or expired"),
        "expected 'invalid or expired' error, got: {}",
        body["error"]
    );
}

// ─── Duplicate email registration ───────────────────────────────────────────

#[tokio::test]
async fn test_register_duplicate_email_returns_409() {
    let app = common::setup().await;

    let email = format!("dup-{}@example.com", Uuid::new_v4());

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
                        "email": email,
                        "password": "securepassword123",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    // Second registration with the same email should return 409
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
                        "email": email,
                        "password": "anotherpassword123",
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 409);
}

// ─── Unauthenticated access to admin endpoints → 401 ────────────────────────

#[tokio::test]
async fn test_admin_endpoints_require_authentication() {
    let app = common::setup().await;
    let random_id = Uuid::new_v4();

    let endpoints: Vec<(&str, String, Option<Value>)> = vec![
        ("POST", "/api/v1/admin/invites".to_string(), Some(json!({}))),
        ("GET", "/api/v1/admin/invites".to_string(), None),
        ("DELETE", format!("/api/v1/admin/invites/{random_id}"), None),
        (
            "PATCH",
            format!("/api/v1/admin/users/{random_id}/status"),
            Some(json!({"status": "disabled"})),
        ),
        ("DELETE", format!("/api/v1/admin/users/{random_id}"), None),
        ("GET", "/api/v1/admin/users".to_string(), None),
        (
            "PATCH",
            format!("/api/v1/admin/users/{random_id}/role"),
            Some(json!({"role": "admin"})),
        ),
    ];

    for (method, uri, body) in &endpoints {
        let mut builder = http::Request::builder().method(*method).uri(uri.as_str());

        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }

        let req_body = match body {
            Some(v) => axum::body::Body::from(serde_json::to_string(v).unwrap()),
            None => axum::body::Body::empty(),
        };

        let response = app
            .app
            .clone()
            .oneshot(builder.body(req_body).unwrap())
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            401,
            "{method} {uri} without auth should return 401, got {}",
            response.status()
        );
    }
}

// ─── Non-admin 403 for all admin endpoints ──────────────────────────────────

#[tokio::test]
async fn test_non_admin_forbidden_on_all_admin_endpoints() {
    let app = common::setup().await;
    let (_user_id, user_token) = common::create_test_user(&app).await;
    let random_id = Uuid::new_v4();

    let endpoints: Vec<(&str, String, Option<Value>)> = vec![
        ("POST", "/api/v1/admin/invites".to_string(), Some(json!({}))),
        ("GET", "/api/v1/admin/invites".to_string(), None),
        ("DELETE", format!("/api/v1/admin/invites/{random_id}"), None),
        (
            "PATCH",
            format!("/api/v1/admin/users/{random_id}/status"),
            Some(json!({"status": "disabled"})),
        ),
        ("DELETE", format!("/api/v1/admin/users/{random_id}"), None),
        ("GET", "/api/v1/admin/users".to_string(), None),
        (
            "PATCH",
            format!("/api/v1/admin/users/{random_id}/role"),
            Some(json!({"role": "admin"})),
        ),
    ];

    for (method, uri, body) in &endpoints {
        let response = app
            .app
            .clone()
            .oneshot(common::auth_request(
                method,
                uri,
                &user_token,
                body.as_ref(),
            ))
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            403,
            "{method} {uri} as non-admin should return 403, got {}",
            response.status()
        );
    }
}

// ─── Delete non-existent user → 404 ─────────────────────────────────────────

#[tokio::test]
async fn test_delete_nonexistent_user_returns_404() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let random_id = Uuid::new_v4();

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/admin/users/{random_id}"),
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

// ─── Invalid status value → 400 ─────────────────────────────────────────────

#[tokio::test]
async fn test_update_user_status_invalid_value_returns_400() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (user_id, _user_token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/admin/users/{user_id}/status"),
            &admin_token,
            Some(&json!({"status": "banned"})),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "invalid status 'banned' should return 400, got {}",
        response.status()
    );
}

// ─── Role update (pre-existing test) ────────────────────────────────────────

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

// ─── Invite claim audit trail ────────────────────────────────────────────────────

#[tokio::test]
async fn test_invite_claim_recorded_on_registration() {
    let app = common::setup_with_config(|c| {
        c.require_invite = true;
    })
    .await;
    let (_admin_id, admin_token) = common::common::create_admin_user(&app).await;

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

    assert_eq!(response.status(), 201);
    let invite_body = common::body_json(response).await;
    let code = invite_body["code"].as_str().unwrap().to_string();
    let invite_id: uuid::Uuid = invite_body["id"].as_str().unwrap().parse().unwrap();

    // Register a user with the invite code
    let email = format!("claimtest-{}@example.com", uuid::Uuid::new_v4());
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/auth/register",
            "",
            Some(&json!({
                "email": email,
                "password": "securepassword123",
                "invite_code": code,
            })),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    // Look up the newly created user's ID
    let user_row: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&email)
        .fetch_one(&app.pool)
        .await
        .expect("user should exist after registration");

    // Verify the invite_claims table has the correct record
    let claim: (uuid::Uuid, uuid::Uuid) =
        sqlx::query_as("SELECT invite_code_id, user_id FROM invite_claims WHERE user_id = $1")
            .bind(user_row.0)
            .fetch_one(&app.pool)
            .await
            .expect("invite_claims row should exist after registration");

    assert_eq!(
        claim.0, invite_id,
        "invite_code_id should match the invite used"
    );
    assert_eq!(
        claim.1, user_row.0,
        "user_id should match the registered user"
    );
}
