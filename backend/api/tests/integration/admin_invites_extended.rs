// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Integration tests for invite check, claims, and stats endpoints.

use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

use crate::common;

// ─── GET /invites/:code/check ────────────────────────────────────────────────

#[tokio::test]
async fn test_check_valid_invite() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    // Create an invite with a label and expiry
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/invites",
            &admin_token,
            Some(&json!({"label": "beta tester", "max_uses": 10, "expires_in_hours": 48})),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 201);
    let invite_body = common::body_json(response).await;
    let code = invite_body["code"].as_str().unwrap();

    // Check the invite code
    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri(&format!("/api/v1/invites/{code}/check"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    assert_eq!(body["valid"], true);
    assert_eq!(body["label"], "beta tester");
    assert!(body["expires_at"].is_string());
    assert!(body["inviter_name"].is_string());
    // reason should not be present when valid
    assert!(body.get("reason").is_none());
}

#[tokio::test]
async fn test_check_nonexistent_invite() {
    let app = common::setup().await;

    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/api/v1/invites/DOESNOTEXIST1234/check")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    assert_eq!(body["valid"], false);
    assert_eq!(body["reason"], "not_found");
}

#[tokio::test]
async fn test_check_revoked_invite() {
    let app = common::setup().await;
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

    // Check the revoked invite
    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri(&format!("/api/v1/invites/{code}/check"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    assert_eq!(body["valid"], false);
    assert_eq!(body["reason"], "revoked");
}

#[tokio::test]
async fn test_check_expired_invite() {
    let app = common::setup().await;
    let (admin_id, _admin_token) = common::create_admin_user(&app).await;

    // Insert an invite directly with expires_at in the past
    let code = format!("EXP{}", &Uuid::new_v4().to_string()[..13]);
    sqlx::query(
        "INSERT INTO invite_codes (code, created_by, expires_at)
         VALUES ($1, $2, now() - interval '1 day')",
    )
    .bind(&code)
    .bind(admin_id)
    .execute(&app.pool)
    .await
    .expect("failed to insert expired invite");

    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri(&format!("/api/v1/invites/{code}/check"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    assert_eq!(body["valid"], false);
    assert_eq!(body["reason"], "expired");
}

#[tokio::test]
async fn test_check_exhausted_invite() {
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

    // Use the invite by registering
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
                        "email": format!("exhaust-{}@example.com", Uuid::new_v4()),
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

    // Now check should return exhausted
    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri(&format!("/api/v1/invites/{code}/check"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    assert_eq!(body["valid"], false);
    assert_eq!(body["reason"], "exhausted");
}

// ─── GET /admin/invites/:id/claims ──────────────────────────────────────────

#[tokio::test]
async fn test_invite_claims_shows_users() {
    let app = common::setup_with_config(|c| {
        c.require_invite = true;
    })
    .await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    // Create an invite
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

    // Register a user with the invite
    let email = format!("claim-test-{}@example.com", Uuid::new_v4());
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
                        "invite_code": code,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    // Fetch claims
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/admin/invites/{invite_id}/claims"),
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    let claims = body.as_array().unwrap();
    assert_eq!(claims.len(), 1);
    // Email should be masked
    let masked = claims[0]["user_email"].as_str().unwrap();
    assert!(
        masked.contains("***@"),
        "email should be masked, got: {masked}"
    );
    assert!(claims[0]["claimed_at"].is_string());
}

#[tokio::test]
async fn test_invite_claims_empty_for_unused_invite() {
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
            Some(&json!({})),
        ))
        .await
        .unwrap();

    let invite_body = common::body_json(response).await;
    let invite_id = invite_body["id"].as_str().unwrap().to_string();

    // Fetch claims for unused invite
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/admin/invites/{invite_id}/claims"),
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    let claims = body.as_array().unwrap();
    assert_eq!(claims.len(), 0);
}

#[tokio::test]
async fn test_invite_claims_requires_auth() {
    let app = common::setup().await;
    let random_id = Uuid::new_v4();

    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri(&format!("/api/v1/admin/invites/{random_id}/claims"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_invite_claims_forbidden_for_non_admin() {
    let app = common::setup().await;
    let (_user_id, user_token) = common::create_test_user(&app).await;
    let random_id = Uuid::new_v4();

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/admin/invites/{random_id}/claims"),
            &user_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 403);
}

// ─── GET /admin/invites/stats ───────────────────────────────────────────────

#[tokio::test]
async fn test_invite_stats() {
    let app = common::setup_with_config(|c| {
        c.require_invite = true;
    })
    .await;
    let (admin_id, admin_token) = common::create_admin_user(&app).await;

    // Create a normal invite (active, unused)
    app.app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/invites",
            &admin_token,
            Some(&json!({})),
        ))
        .await
        .unwrap();

    // Create an invite and use it (used + active)
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

    // Register a user to make it "used"
    app.app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri("/api/v1/auth/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_string(&json!({
                        "email": format!("stats-{}@example.com", Uuid::new_v4()),
                        "password": "securepassword123",
                        "invite_code": code,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

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
    let invite_id = invite_body["id"].as_str().unwrap().to_string();

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

    // Create an expired invite directly
    sqlx::query(
        "INSERT INTO invite_codes (code, created_by, expires_at)
         VALUES ($1, $2, now() - interval '1 day')",
    )
    .bind(format!("EXPSTATS{}", &Uuid::new_v4().to_string()[..8]))
    .bind(admin_id)
    .execute(&app.pool)
    .await
    .expect("failed to insert expired invite");

    // Fetch stats
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/admin/invites/stats",
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    assert_eq!(body["total"], 4); // 1 active + 1 used + 1 revoked + 1 expired
    assert_eq!(body["active"], 2); // the active unused one + the used one (still valid)
    assert_eq!(body["used"], 1); // only the one that was actually claimed
    assert_eq!(body["revoked"], 1);
    assert_eq!(body["expired"], 1);
}

#[tokio::test]
async fn test_invite_stats_requires_auth() {
    let app = common::setup().await;

    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/api/v1/admin/invites/stats")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_invite_stats_forbidden_for_non_admin() {
    let app = common::setup().await;
    let (_user_id, user_token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/admin/invites/stats",
            &user_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 403);
}
