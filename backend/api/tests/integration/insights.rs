// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{NaiveDate, Utc};
use tower::ServiceExt;
use uuid::Uuid;

use crate::common;

// ---------------------------------------------------------------------------
// List — empty state
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_insights_empty() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/insights",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let json = common::body_json(response).await;
    let items = json.as_array().unwrap();
    assert!(items.is_empty());
}

// ---------------------------------------------------------------------------
// Unauthenticated access → 401
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_insights_unauthenticated() {
    let app = common::setup().await;

    let request = http::Request::builder()
        .method("GET")
        .uri("/api/v1/insights")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = app.app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_generate_unauthenticated() {
    let app = common::setup().await;

    let request = http::Request::builder()
        .method("POST")
        .uri("/api/v1/insights/generate")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = app.app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_dismiss_unauthenticated() {
    let app = common::setup().await;
    let id = Uuid::new_v4();

    let request = http::Request::builder()
        .method("POST")
        .uri(&format!("/api/v1/insights/{id}/dismiss"))
        .body(axum::body::Body::empty())
        .unwrap();

    let response = app.app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 401);
}

// ---------------------------------------------------------------------------
// Generate — no data → empty result
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_generate_no_data_returns_empty() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/insights/generate",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let json = common::body_json(response).await;
    let items = json.as_array().unwrap();
    assert!(items.is_empty());
}

// ---------------------------------------------------------------------------
// Generate with checkin data → produces missing-data insight
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_generate_missing_data_insight() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    // Insert 5 check-ins, all older than 3 days
    for i in 10..15 {
        let date = NaiveDate::from_ymd_opt(2026, 3, i).unwrap();
        sqlx::query("INSERT INTO daily_checkins (user_id, date, energy) VALUES ($1, $2, 7)")
            .bind(user_id)
            .bind(date)
            .execute(&app.pool)
            .await
            .unwrap();
    }

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/insights/generate",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let json = common::body_json(response).await;
    let items = json.as_array().unwrap();

    // Should have a missing_data insight
    let missing = items.iter().find(|i| i["insight_type"] == "missing_data");
    assert!(
        missing.is_some(),
        "expected a missing_data insight, got: {items:?}"
    );

    let insight = missing.unwrap();
    assert!(insight["headline"].as_str().unwrap().contains("check-in"));
    assert!(insight["id"].as_str().is_some());
    assert!(insight["dismissed_at"].is_null());
}

// ---------------------------------------------------------------------------
// Dismiss — happy path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dismiss_insight() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    // Insert an insight directly
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO insights (user_id, insight_type, headline, metadata)
         VALUES ($1, 'test', 'Test insight', '{}')
         RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    let dismiss_response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/insights/{}/dismiss", row.0),
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(dismiss_response.status(), 204);

    // Listing should now be empty (dismissed insights are hidden)
    let list_response = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/insights",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(list_response.status(), 200);
    let json = common::body_json(list_response).await;
    assert!(json.as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Dismiss — not found (nonexistent ID)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dismiss_not_found() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/insights/{}/dismiss", Uuid::new_v4()),
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

// ---------------------------------------------------------------------------
// IDOR — user A cannot dismiss user B's insight
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dismiss_idor_protection() {
    let app = common::setup().await;
    let (user_a_id, _token_a) = common::create_test_user(&app).await;
    let (_user_b_id, token_b) = common::create_test_user(&app).await;

    // Insert insight for user A
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO insights (user_id, insight_type, headline, metadata)
         VALUES ($1, 'test', 'User A insight', '{}')
         RETURNING id",
    )
    .bind(user_a_id)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    // User B tries to dismiss user A's insight
    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/insights/{}/dismiss", row.0),
            &token_b,
            None,
        ))
        .await
        .unwrap();

    // Should not be found for user B (IDOR protection)
    assert_eq!(response.status(), 404);
}

// ---------------------------------------------------------------------------
// List — only shows own insights
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_only_own_insights() {
    let app = common::setup().await;
    let (user_a_id, token_a) = common::create_test_user(&app).await;
    let (user_b_id, _token_b) = common::create_test_user(&app).await;

    // Insert insight for user A
    sqlx::query(
        "INSERT INTO insights (user_id, insight_type, headline, metadata)
         VALUES ($1, 'test', 'User A insight', '{}')",
    )
    .bind(user_a_id)
    .execute(&app.pool)
    .await
    .unwrap();

    // Insert insight for user B
    sqlx::query(
        "INSERT INTO insights (user_id, insight_type, headline, metadata)
         VALUES ($1, 'test', 'User B insight', '{}')",
    )
    .bind(user_b_id)
    .execute(&app.pool)
    .await
    .unwrap();

    // User A should only see their own insight
    let response = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/insights",
            &token_a,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let json = common::body_json(response).await;
    let items = json.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["headline"], "User A insight");
}

// ---------------------------------------------------------------------------
// Generate streak insight with 7 consecutive days
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_generate_streak_insight() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    // Insert 7 consecutive days of check-ins ending today
    let today = Utc::now().date_naive();
    for i in 0..7 {
        let date = today - chrono::Duration::days(i);
        sqlx::query("INSERT INTO daily_checkins (user_id, date, energy) VALUES ($1, $2, 7)")
            .bind(user_id)
            .bind(date)
            .execute(&app.pool)
            .await
            .unwrap();
    }

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/insights/generate",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let json = common::body_json(response).await;
    let items = json.as_array().unwrap();

    let streak = items.iter().find(|i| i["insight_type"] == "streak");
    assert!(
        streak.is_some(),
        "expected a streak insight for 7 consecutive days, got: {items:?}"
    );

    let insight = streak.unwrap();
    assert!(
        insight["headline"]
            .as_str()
            .unwrap()
            .contains("7 consecutive days")
    );
}

// ---------------------------------------------------------------------------
// Deduplication — generating twice does not produce duplicates
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_generate_deduplication() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    // Insert data that will produce a missing_data insight
    for i in 10..15 {
        let date = NaiveDate::from_ymd_opt(2026, 3, i).unwrap();
        sqlx::query("INSERT INTO daily_checkins (user_id, date, energy) VALUES ($1, $2, 7)")
            .bind(user_id)
            .bind(date)
            .execute(&app.pool)
            .await
            .unwrap();
    }

    // First generation
    let resp1 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/insights/generate",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp1.status(), 200);
    let json1 = common::body_json(resp1).await;
    let count1 = json1
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["insight_type"] == "missing_data")
        .count();

    // Second generation — should not produce another missing_data
    let resp2 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/insights/generate",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp2.status(), 200);
    let json2 = common::body_json(resp2).await;
    let count2 = json2
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["insight_type"] == "missing_data")
        .count();

    // First should have produced one, second should have produced zero
    assert!(count1 > 0, "first generation should produce missing_data");
    assert_eq!(count2, 0, "second generation should be deduplicated");
}
