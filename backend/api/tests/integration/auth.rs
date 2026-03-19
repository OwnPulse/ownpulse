// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::body::Body;
use http::Request;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::common;

/// Helper: collect the response body into a parsed JSON value.
async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
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

/// Helper: build a POST request with a cookie header.
fn post_with_cookie(uri: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap()
}

/// Helper: insert a local user into the database with a bcrypt-hashed password.
async fn insert_test_user(pool: &sqlx::PgPool, username: &str, password: &str) -> uuid::Uuid {
    let hash = bcrypt::hash(password, 4).expect("bcrypt hash failed");
    let row: (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO users (username, password_hash, auth_provider) VALUES ($1, $2, 'local') RETURNING id",
    )
    .bind(username)
    .bind(&hash)
    .fetch_one(pool)
    .await
    .expect("failed to insert test user");
    row.0
}

/// Helper: extract the refresh_token value from a Set-Cookie header.
fn extract_refresh_cookie(response: &axum::response::Response) -> String {
    let cookie_header = response
        .headers()
        .get("set-cookie")
        .expect("no set-cookie header")
        .to_str()
        .unwrap();
    cookie_header
        .split(';')
        .next()
        .unwrap()
        .strip_prefix("refresh_token=")
        .expect("cookie does not start with refresh_token=")
        .to_string()
}

#[tokio::test]
async fn test_login_with_valid_credentials() {
    let test_app = common::setup().await;
    insert_test_user(&test_app.pool, "alice", "correctpassword").await;

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"username": "alice", "password": "correctpassword"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    // Verify Set-Cookie header contains refresh_token
    let cookie_value = extract_refresh_cookie(&response);
    assert!(!cookie_value.is_empty(), "refresh_token cookie should not be empty");

    let json = body_json(response).await;
    assert!(json["access_token"].is_string());
    assert!(!json["access_token"].as_str().unwrap().is_empty());
    assert_eq!(json["token_type"], "Bearer");
}

#[tokio::test]
async fn test_login_with_wrong_password() {
    let test_app = common::setup().await;
    insert_test_user(&test_app.pool, "bob", "realpassword").await;

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"username": "bob", "password": "wrongpassword"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_login_with_nonexistent_user() {
    let test_app = common::setup().await;

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"username": "nobody", "password": "whatever"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_refresh_token_rotation() {
    let test_app = common::setup().await;
    insert_test_user(&test_app.pool, "carol", "mypassword").await;

    // Login to get the refresh cookie
    let login_response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"username": "carol", "password": "mypassword"}),
        ))
        .await
        .unwrap();

    assert_eq!(login_response.status(), 200);
    let refresh_token = extract_refresh_cookie(&login_response);

    // Use the refresh cookie to get a new access token
    let refresh_response = test_app
        .app
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={refresh_token}"),
        ))
        .await
        .unwrap();

    assert_eq!(refresh_response.status(), 200);

    // Verify we got a new access token and a new refresh cookie
    let new_refresh = extract_refresh_cookie(&refresh_response);
    assert!(!new_refresh.is_empty());
    assert_ne!(refresh_token, new_refresh, "refresh token should be rotated");

    let json = body_json(refresh_response).await;
    assert!(json["access_token"].is_string());
    assert!(!json["access_token"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_refresh_with_no_cookie() {
    let test_app = common::setup().await;

    let response = test_app
        .app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_logout_clears_refresh_token() {
    let test_app = common::setup().await;
    insert_test_user(&test_app.pool, "dave", "secret123").await;

    // Login
    let login_response = test_app
        .app
        .clone()
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"username": "dave", "password": "secret123"}),
        ))
        .await
        .unwrap();

    assert_eq!(login_response.status(), 200);
    let refresh_token = extract_refresh_cookie(&login_response);

    // Logout with that refresh cookie
    let logout_response = test_app
        .app
        .clone()
        .oneshot(post_with_cookie(
            "/api/v1/auth/logout",
            &format!("refresh_token={refresh_token}"),
        ))
        .await
        .unwrap();

    assert_eq!(logout_response.status(), 204);

    // Try to refresh with the old token — should fail
    let refresh_response = test_app
        .app
        .oneshot(post_with_cookie(
            "/api/v1/auth/refresh",
            &format!("refresh_token={refresh_token}"),
        ))
        .await
        .unwrap();

    assert_eq!(refresh_response.status(), 401);
}

#[tokio::test]
async fn test_login_returns_decodable_jwt() {
    let test_app = common::setup().await;
    let user_id = insert_test_user(&test_app.pool, "eve", "jwttest").await;

    let response = test_app
        .app
        .oneshot(post_json(
            "/api/v1/auth/login",
            &json!({"username": "eve", "password": "jwttest"}),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let json = body_json(response).await;
    let access_token = json["access_token"].as_str().unwrap();

    // Decode the JWT using the same secret as the test config
    let claims = api::auth::jwt::decode_access_token(
        access_token,
        "test-jwt-secret-at-least-32-bytes-long",
    )
    .expect("JWT should decode successfully");

    assert_eq!(claims.sub, user_id);
    assert!(claims.exp > chrono::Utc::now().timestamp());
}
