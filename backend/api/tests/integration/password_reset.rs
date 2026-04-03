// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::body::Body;
use chrono::{Duration, Utc};
use http::Request;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

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

/// Helper: insert a local user with a bcrypt-hashed password.
async fn insert_local_user(pool: &sqlx::PgPool, email: &str, password: &str) -> Uuid {
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
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject) VALUES ($1, 'local', $2)",
    )
    .bind(row.0)
    .bind(row.0.to_string())
    .execute(pool)
    .await
    .expect("failed to insert user_auth_methods row");

    row.0
}

/// Helper: insert an OAuth-only user (no password_hash).
async fn insert_oauth_user(pool: &sqlx::PgPool, email: &str) -> Uuid {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO users (email, auth_provider) VALUES ($1, 'google') RETURNING id",
    )
    .bind(email)
    .fetch_one(pool)
    .await
    .expect("failed to insert oauth user");

    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject) VALUES ($1, 'google', $2)",
    )
    .bind(row.0)
    .bind(format!("google-{}", row.0))
    .execute(pool)
    .await
    .expect("failed to insert user_auth_methods row");

    row.0
}

/// Helper: insert a password reset token directly into the DB.
/// Returns the raw token string that the client would use.
fn hash_token(raw_token: &str) -> String {
    api::auth::refresh::hash_refresh_token(raw_token, "test-jwt-secret-at-least-32-bytes-long")
}

/// Helper: insert a password reset token row with custom expiry and claimed_at.
async fn insert_reset_token(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    token_hash: &str,
    expires_at: chrono::DateTime<Utc>,
    claimed_at: Option<chrono::DateTime<Utc>>,
) {
    sqlx::query(
        "INSERT INTO password_reset_tokens (user_id, token_hash, expires_at, claimed_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .bind(claimed_at)
    .execute(pool)
    .await
    .expect("failed to insert reset token");
}

// ── Forgot password tests ───────────────────────────────────────────

#[tokio::test]
async fn test_forgot_password_returns_200_existing_user() {
    let app = common::setup().await;
    let email = format!("reset-{}@example.com", Uuid::new_v4());
    let user_id = insert_local_user(&app.pool, &email, "validpassword123").await;

    let response = app
        .app
        .oneshot(post_json(
            "/api/v1/auth/forgot-password",
            &json!({"email": email}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    // Verify a token row was created in the DB
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM password_reset_tokens WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .expect("failed to query password_reset_tokens");

    assert_eq!(count.0, 1, "should have created exactly one reset token");
}

#[tokio::test]
async fn test_forgot_password_returns_200_nonexistent_email() {
    let app = common::setup().await;

    let response = app
        .app
        .oneshot(post_json(
            "/api/v1/auth/forgot-password",
            &json!({"email": "nobody@example.com"}),
        ))
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        200,
        "should return 200 to prevent email enumeration"
    );

    // Verify NO token row exists
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM password_reset_tokens")
            .fetch_one(&app.pool)
            .await
            .expect("failed to query password_reset_tokens");

    assert_eq!(count.0, 0, "should not create any token for nonexistent email");
}

#[tokio::test]
async fn test_forgot_password_returns_200_oauth_user() {
    let app = common::setup().await;
    let email = format!("oauth-{}@example.com", Uuid::new_v4());
    insert_oauth_user(&app.pool, &email).await;

    let response = app
        .app
        .oneshot(post_json(
            "/api/v1/auth/forgot-password",
            &json!({"email": email}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    // No reset token should be created for OAuth users
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM password_reset_tokens")
            .fetch_one(&app.pool)
            .await
            .expect("failed to query password_reset_tokens");

    assert_eq!(
        count.0, 0,
        "should not create reset token for OAuth-only user"
    );
}

#[tokio::test]
async fn test_forgot_password_invalidates_previous_tokens() {
    let app = common::setup().await;
    let email = format!("reset-twice-{}@example.com", Uuid::new_v4());
    let user_id = insert_local_user(&app.pool, &email, "validpassword123").await;

    // First request
    let response = app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/forgot-password",
            &json!({"email": email}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // Second request
    let response = app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/forgot-password",
            &json!({"email": email}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    // Should have 2 tokens total, but only 1 unclaimed
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM password_reset_tokens WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    let unclaimed: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM password_reset_tokens WHERE user_id = $1 AND claimed_at IS NULL",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    assert_eq!(total.0, 2, "should have two token rows total");
    assert_eq!(
        unclaimed.0, 1,
        "only the latest token should be unclaimed"
    );
}

// ── Reset password tests ────────────────────────────────────────────

#[tokio::test]
async fn test_reset_password_valid_token() {
    let app = common::setup().await;
    let email = format!("reset-valid-{}@example.com", Uuid::new_v4());
    let user_id = insert_local_user(&app.pool, &email, "oldpassword123").await;

    // Insert a token directly
    let raw_token = Uuid::new_v4().to_string();
    let token_hash = hash_token(&raw_token);
    let expires_at = Utc::now() + Duration::hours(1);
    insert_reset_token(&app.pool, user_id, &token_hash, expires_at, None).await;

    let response = app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/reset-password",
            &json!({"token": raw_token, "password": "newstrongpassword123"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    // Verify password was actually changed by logging in with new password
    let response = app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"email": email, "password": "newstrongpassword123"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200, "should be able to login with new password");
}

#[tokio::test]
async fn test_reset_password_expired_token() {
    let app = common::setup().await;
    let email = format!("reset-expired-{}@example.com", Uuid::new_v4());
    let user_id = insert_local_user(&app.pool, &email, "somepassword123").await;

    let raw_token = Uuid::new_v4().to_string();
    let token_hash = hash_token(&raw_token);
    let expires_at = Utc::now() - Duration::hours(1); // expired
    insert_reset_token(&app.pool, user_id, &token_hash, expires_at, None).await;

    let response = app
        .app
        .oneshot(post_json(
            "/api/v1/auth/reset-password",
            &json!({"token": raw_token, "password": "newstrongpassword123"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_reset_password_already_used_token() {
    let app = common::setup().await;
    let email = format!("reset-used-{}@example.com", Uuid::new_v4());
    let user_id = insert_local_user(&app.pool, &email, "somepassword123").await;

    let raw_token = Uuid::new_v4().to_string();
    let token_hash = hash_token(&raw_token);
    let expires_at = Utc::now() + Duration::hours(1);
    let claimed_at = Some(Utc::now() - Duration::minutes(30));
    insert_reset_token(&app.pool, user_id, &token_hash, expires_at, claimed_at).await;

    let response = app
        .app
        .oneshot(post_json(
            "/api/v1/auth/reset-password",
            &json!({"token": raw_token, "password": "newstrongpassword123"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_reset_password_invalid_token() {
    let app = common::setup().await;

    let response = app
        .app
        .oneshot(post_json(
            "/api/v1/auth/reset-password",
            &json!({"token": Uuid::new_v4().to_string(), "password": "newstrongpassword123"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_reset_password_short_password() {
    let app = common::setup().await;
    let email = format!("reset-short-{}@example.com", Uuid::new_v4());
    let user_id = insert_local_user(&app.pool, &email, "somepassword123").await;

    let raw_token = Uuid::new_v4().to_string();
    let token_hash = hash_token(&raw_token);
    let expires_at = Utc::now() + Duration::hours(1);
    insert_reset_token(&app.pool, user_id, &token_hash, expires_at, None).await;

    let response = app
        .app
        .oneshot(post_json(
            "/api/v1/auth/reset-password",
            &json!({"token": raw_token, "password": "short"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
    let json = body_json(response).await;
    assert!(
        json["error"]
            .as_str()
            .unwrap_or("")
            .contains("10 characters"),
        "error should mention 10 character minimum"
    );
}

#[tokio::test]
async fn test_reset_password_revokes_sessions() {
    let app = common::setup().await;
    let email = format!("reset-revoke-{}@example.com", Uuid::new_v4());
    let user_id = insert_local_user(&app.pool, &email, "oldpassword123").await;

    // Insert some refresh tokens to simulate active sessions
    let refresh_hash = hash_token("fake-refresh-1");
    sqlx::query(
        "INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(&refresh_hash)
    .bind(Utc::now() + Duration::days(30))
    .execute(&app.pool)
    .await
    .expect("failed to insert refresh token");

    let refresh_hash_2 = hash_token("fake-refresh-2");
    sqlx::query(
        "INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(&refresh_hash_2)
    .bind(Utc::now() + Duration::days(30))
    .execute(&app.pool)
    .await
    .expect("failed to insert second refresh token");

    // Verify refresh tokens exist
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(count.0, 2, "should have 2 refresh tokens before reset");

    // Reset password
    let raw_token = Uuid::new_v4().to_string();
    let token_hash = hash_token(&raw_token);
    let expires_at = Utc::now() + Duration::hours(1);
    insert_reset_token(&app.pool, user_id, &token_hash, expires_at, None).await;

    let response = app
        .app
        .oneshot(post_json(
            "/api/v1/auth/reset-password",
            &json!({"token": raw_token, "password": "newstrongpassword123"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    // All refresh tokens should be deleted
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert_eq!(
        count.0, 0,
        "all refresh tokens should be revoked after password reset"
    );
}

// ── Password validation tests ───────────────────────────────────────

#[tokio::test]
async fn test_register_short_password_returns_400() {
    let app = common::setup().await;

    let response = app
        .app
        .oneshot(post_json(
            "/api/v1/auth/register",
            &json!({
                "email": format!("short-pw-{}@example.com", Uuid::new_v4()),
                "password": "12345"
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
    let json = body_json(response).await;
    assert!(
        json["error"]
            .as_str()
            .unwrap_or("")
            .contains("10 characters"),
        "error should mention 10 character minimum"
    );
}

#[tokio::test]
async fn test_reset_password_short_password_returns_400() {
    let app = common::setup().await;
    let email = format!("reset-short2-{}@example.com", Uuid::new_v4());
    let user_id = insert_local_user(&app.pool, &email, "somepassword123").await;

    let raw_token = Uuid::new_v4().to_string();
    let token_hash = hash_token(&raw_token);
    let expires_at = Utc::now() + Duration::hours(1);
    insert_reset_token(&app.pool, user_id, &token_hash, expires_at, None).await;

    let response = app
        .app
        .oneshot(post_json(
            "/api/v1/auth/reset-password",
            &json!({"token": raw_token, "password": "12345"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
    let json = body_json(response).await;
    assert!(
        json["error"]
            .as_str()
            .unwrap_or("")
            .contains("10 characters"),
        "error should mention 10 character minimum"
    );
}
