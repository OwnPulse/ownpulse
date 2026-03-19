// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::body::Body;
use http::Request;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::common;

/// Helper: build a POST /api/v1/waitlist request with the given JSON body.
fn waitlist_request(body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/v1/waitlist")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .unwrap()
}

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_signup_valid_email() {
    let test_app = common::setup().await;

    let response = test_app
        .app
        .oneshot(waitlist_request(&json!({"email": "test@example.com"})))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let json = body_json(response).await;
    assert_eq!(json["ok"], true);

    // Verify the row was persisted.
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM waitlist WHERE email = 'test@example.com'")
            .fetch_one(&test_app.pool)
            .await
            .unwrap();
    assert_eq!(row.0, 1);
}

#[tokio::test]
async fn test_signup_duplicate_email() {
    let test_app = common::setup().await;

    // First signup.
    let resp1 = test_app
        .app
        .clone()
        .oneshot(waitlist_request(&json!({"email": "dup@example.com"})))
        .await
        .unwrap();
    assert_eq!(resp1.status(), 200);

    // Second signup with the same email.
    let resp2 = test_app
        .app
        .oneshot(waitlist_request(&json!({"email": "dup@example.com"})))
        .await
        .unwrap();
    assert_eq!(resp2.status(), 200);

    let json = body_json(resp2).await;
    assert_eq!(json["ok"], true);

    // Only one row should exist.
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM waitlist WHERE email = 'dup@example.com'")
            .fetch_one(&test_app.pool)
            .await
            .unwrap();
    assert_eq!(row.0, 1);
}

#[tokio::test]
async fn test_signup_empty_email() {
    let test_app = common::setup().await;

    let response = test_app
        .app
        .oneshot(waitlist_request(&json!({"email": ""})))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);

    let json = body_json(response).await;
    assert_eq!(json["ok"], false);
}

#[tokio::test]
async fn test_signup_no_at_sign() {
    let test_app = common::setup().await;

    let response = test_app
        .app
        .oneshot(waitlist_request(&json!({"email": "notanemail"})))
        .await
        .unwrap();

    assert_eq!(response.status(), 400);

    let json = body_json(response).await;
    assert_eq!(json["ok"], false);
}

#[tokio::test]
async fn test_signup_with_name() {
    let test_app = common::setup().await;

    let response = test_app
        .app
        .oneshot(waitlist_request(&json!({"email": "a@b.com", "name": "Alice"})))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let json = body_json(response).await;
    assert_eq!(json["ok"], true);

    // Verify the name was stored.
    let row: (Option<String>,) =
        sqlx::query_as("SELECT name FROM waitlist WHERE email = 'a@b.com'")
            .fetch_one(&test_app.pool)
            .await
            .unwrap();
    assert_eq!(row.0.as_deref(), Some("Alice"));
}

#[tokio::test]
async fn test_signup_without_name() {
    let test_app = common::setup().await;

    let response = test_app
        .app
        .oneshot(waitlist_request(&json!({"email": "a@b.com"})))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let json = body_json(response).await;
    assert_eq!(json["ok"], true);

    // Verify the name is NULL.
    let row: (Option<String>,) =
        sqlx::query_as("SELECT name FROM waitlist WHERE email = 'a@b.com'")
            .fetch_one(&test_app.pool)
            .await
            .unwrap();
    assert!(row.0.is_none());
}

#[tokio::test]
async fn test_signup_email_trimmed_and_lowercased() {
    let test_app = common::setup().await;

    let response = test_app
        .app
        .oneshot(waitlist_request(&json!({"email": "  Test@Example.COM  "})))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let json = body_json(response).await;
    assert_eq!(json["ok"], true);

    // Verify it was stored as the normalized form.
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM waitlist WHERE email = 'test@example.com'")
            .fetch_one(&test_app.pool)
            .await
            .unwrap();
    assert_eq!(row.0, 1);
}
