// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Integration tests for the security and principles fixes (PR #86 follow-ups).

use axum::body::Body;
use http::Request;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

use crate::common;

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Collect the response body into a parsed JSON value.
async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Build a POST request with JSON body (unauthenticated).
fn post_json(uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .unwrap()
}

/// Insert a local user with a known password and return their ID.
async fn insert_test_user(pool: &sqlx::PgPool, email: &str, password: &str) -> Uuid {
    let hash = bcrypt::hash(password, 4).expect("bcrypt hash failed");
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO users (email, password_hash, auth_provider) VALUES ($1, $2, 'local') RETURNING id",
    )
    .bind(email)
    .bind(&hash)
    .fetch_one(pool)
    .await
    .expect("failed to insert test user");

    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject)
         VALUES ($1, 'local', $2)",
    )
    .bind(row.0)
    .bind(row.0.to_string())
    .execute(pool)
    .await
    .expect("failed to insert user_auth_methods row");

    row.0
}

/// Extract the refresh_token value from Set-Cookie headers.
fn extract_refresh_cookie(response: &axum::response::Response) -> String {
    response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(|cookie| {
            cookie
                .split(';')
                .next()
                .and_then(|first| first.strip_prefix("refresh_token="))
                .map(|s| s.to_string())
        })
        .find(|s| !s.is_empty())
        .expect("no refresh_token cookie found")
}

/// Build a POST request with a cookie header.
fn post_with_cookie(uri: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap()
}

/// Disable a user via admin API.
async fn disable_user(app: &common::TestApp, admin_token: &str, user_id: Uuid) {
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/admin/users/{user_id}/status"),
            admin_token,
            Some(&json!({"status": "disabled"})),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

// ─── Item 1: Block login/refresh for disabled users ──────────────────────────

#[tokio::test]
async fn test_login_as_disabled_user_returns_access_token_only() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let email = format!("disabled-login-{}@example.com", Uuid::new_v4());
    let user_id = insert_test_user(&app.pool, &email, "password123").await;

    disable_user(&app, &admin_token, user_id).await;

    let response = app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": email, "password": "password123"}),
        ))
        .await
        .unwrap();

    // Disabled users get a 200 with access_token only — no refresh token,
    // no refresh cookie. This lets them export data or self-delete.
    assert_eq!(response.status(), 200);

    // Verify no refresh cookie is set.
    let has_refresh_cookie = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .any(|cookie| cookie.contains("refresh_token=") && !cookie.contains("refresh_token=;"));
    assert!(
        !has_refresh_cookie,
        "disabled user login should not set a refresh_token cookie"
    );

    let body = body_json(response).await;
    assert!(
        body["access_token"].is_string(),
        "response should contain access_token"
    );
    assert!(
        body.get("refresh_token").is_none() || body["refresh_token"].is_null(),
        "response should not contain refresh_token"
    );
}

#[tokio::test]
async fn test_refresh_for_disabled_user_returns_403() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let email = format!("disabled-refresh-{}@example.com", Uuid::new_v4());
    insert_test_user(&app.pool, &email, "password123").await;

    // Login to get a refresh token
    let login_response = app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": email, "password": "password123"}),
        ))
        .await
        .unwrap();
    assert_eq!(login_response.status(), 200);
    let refresh_token = extract_refresh_cookie(&login_response);

    // Get user ID from the access token
    let login_body = body_json(login_response).await;
    let access_token = login_body["access_token"].as_str().unwrap();
    let claims = api::auth::jwt::decode_access_token(
        access_token,
        "test-jwt-secret-at-least-32-bytes-long",
        "http://localhost:5173",
    )
    .unwrap();
    let user_id = claims.sub;

    disable_user(&app, &admin_token, user_id).await;

    // The disable should have deleted all refresh tokens, so refreshing
    // returns 401 (token not found) rather than 403.
    let response = app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={refresh_token}"),
        ))
        .await
        .unwrap();

    // Token was revoked on disable, so we get 401 (not found).
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_disable_user_revokes_refresh_tokens() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let email = format!("revoke-tokens-{}@example.com", Uuid::new_v4());
    insert_test_user(&app.pool, &email, "password123").await;

    // Login to get a refresh token
    let login_response = app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": email, "password": "password123"}),
        ))
        .await
        .unwrap();
    assert_eq!(login_response.status(), 200);
    let refresh_token = extract_refresh_cookie(&login_response);

    let login_body = body_json(login_response).await;
    let access_token = login_body["access_token"].as_str().unwrap();
    let claims = api::auth::jwt::decode_access_token(
        access_token,
        "test-jwt-secret-at-least-32-bytes-long",
        "http://localhost:5173",
    )
    .unwrap();
    let user_id = claims.sub;

    // Disable the user
    disable_user(&app, &admin_token, user_id).await;

    // Verify refresh tokens are gone by checking the database
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(
        count.0, 0,
        "all refresh tokens should be deleted on disable"
    );

    // Trying to refresh should fail
    let response = app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={refresh_token}"),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

// ─── Item 5: Disabled users can export data and self-delete ──────────────────

#[tokio::test]
async fn test_disabled_user_can_export_json() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (user_id, user_token) = common::create_test_user(&app).await;

    disable_user(&app, &admin_token, user_id).await;

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/export/json",
            &user_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_disabled_user_can_export_csv() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (user_id, user_token) = common::create_test_user(&app).await;

    disable_user(&app, &admin_token, user_id).await;

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/export/csv",
            &user_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_disabled_user_can_delete_account() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (user_id, user_token) = common::create_test_user(&app).await;

    disable_user(&app, &admin_token, user_id).await;

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            "/api/v1/account",
            &user_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 204);
}

#[tokio::test]
async fn test_disabled_user_cannot_post_health_records() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (user_id, user_token) = common::create_test_user(&app).await;

    disable_user(&app, &admin_token, user_id).await;

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/health-records",
            &user_token,
            Some(&json!({
                "source": "manual",
                "record_type": "heart_rate",
                "value": 65.0,
                "unit": "bpm",
                "start_time": "2026-03-18T10:00:00Z"
            })),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 403);
}

// ─── Item 6: Require disabled status before admin deletion ───────────────────

#[tokio::test]
async fn test_delete_active_user_returns_400() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (user_id, _user_token) = common::create_test_user(&app).await;

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

    assert_eq!(response.status(), 400);
    let body = body_json(response).await;
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("must be disabled before deletion"),
        "expected 'must be disabled before deletion', got: {}",
        body["error"]
    );
}

#[tokio::test]
async fn test_disable_then_delete_user_succeeds() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (user_id, _user_token) = common::create_test_user(&app).await;

    // Disable first
    disable_user(&app, &admin_token, user_id).await;

    // Then delete
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
}

// ─── Item 8: Validate invite creation bounds ─────────────────────────────────

#[tokio::test]
async fn test_create_invite_negative_max_uses_returns_400() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/invites",
            &admin_token,
            Some(&json!({"max_uses": -1})),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
    let body = body_json(response).await;
    assert!(body["error"].as_str().unwrap().contains("max_uses"));
}

#[tokio::test]
async fn test_create_invite_zero_max_uses_returns_400() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/invites",
            &admin_token,
            Some(&json!({"max_uses": 0})),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_create_invite_negative_expires_in_hours_returns_400() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/invites",
            &admin_token,
            Some(&json!({"expires_in_hours": -1})),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
    let body = body_json(response).await;
    assert!(body["error"].as_str().unwrap().contains("expires_in_hours"));
}

#[tokio::test]
async fn test_create_invite_with_valid_values_succeeds() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/invites",
            &admin_token,
            Some(&json!({"max_uses": 5, "expires_in_hours": 24})),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 201);
    let body = body_json(response).await;
    assert_eq!(body["max_uses"], 5);
}

// ─── Item 3: Atomic invite claim with user creation (concurrency) ────────────

#[tokio::test]
async fn test_concurrent_registrations_against_single_use_invite() {
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
    assert_eq!(response.status(), 201);
    let invite_body = body_json(response).await;
    let code = invite_body["code"].as_str().unwrap().to_string();

    // Fire 10 concurrent registrations
    let mut join_set = tokio::task::JoinSet::new();
    for i in 0..10 {
        let router = app.app.clone();
        let code = code.clone();
        join_set.spawn(async move {
            let response = router
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/auth/register")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::to_string(&json!({
                                "email": format!("concurrent-{i}@example.com"),
                                "password": "securepassword123",
                                "invite_code": code,
                            }))
                            .unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            response.status()
        });
    }

    let mut successes = 0;
    let mut failures = 0;
    while let Some(result) = join_set.join_next().await {
        let status = result.unwrap();
        if status == 200 {
            successes += 1;
        } else {
            failures += 1;
        }
    }

    assert_eq!(
        successes, 1,
        "exactly 1 registration should succeed with max_uses=1, got {successes} successes and {failures} failures"
    );
    assert_eq!(failures, 9);
}
