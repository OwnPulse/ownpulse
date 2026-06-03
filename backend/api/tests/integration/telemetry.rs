// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use http::StatusCode;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::common::{auth_request, body_json, create_test_user, setup};

/// Poll app_events until at least `expected` rows of `event_type` exist, since
/// the handler persists telemetry via a detached `tokio::spawn`.
async fn wait_for_events(pool: &sqlx::PgPool, event_type: &str, expected: i64) -> Vec<(String,)> {
    for _ in 0..50 {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT platform FROM app_events WHERE event_type = $1 ORDER BY id")
                .bind(event_type)
                .fetch_all(pool)
                .await
                .expect("query app_events");
        if rows.len() as i64 >= expected {
            return rows;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("timed out waiting for {expected} {event_type} event(s)");
}

#[tokio::test]
async fn api_call_event_stored_with_platform() {
    let app = setup().await;
    let (_uid, token) = create_test_user(&app).await;

    let body = json!({
        "events": [{
            "type": "api_call",
            "device_id": "device-abc",
            "platform": "web",
            "app_version": "1.2.3",
            "payload": {
                "endpoint": "/protocols/42/runs",
                "method": "POST",
                "status": 201,
                "latency_ms": 87,
                "retry_count": 0
            }
        }]
    });

    let resp = app
        .app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/v1/telemetry/report",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json: Value = body_json(resp).await;
    assert_eq!(json["accepted"], 1);
    assert_eq!(json["rejected"], 0);

    let rows = wait_for_events(&app.pool, "api_call", 1).await;
    assert_eq!(rows[0].0, "web");

    // The stored payload must contain only allowlisted fields.
    let stored: (Value,) =
        sqlx::query_as("SELECT payload FROM app_events WHERE event_type = 'api_call' LIMIT 1")
            .fetch_one(&app.pool)
            .await
            .unwrap();
    let obj = stored.0.as_object().unwrap();
    // Endpoint must be normalized — the `42` path-segment ID is stripped.
    assert_eq!(
        obj.get("endpoint").and_then(|v| v.as_str()),
        Some("/protocols/:id/runs")
    );
    assert!(obj.contains_key("method"));
    assert!(obj.contains_key("status"));
    assert!(obj.contains_key("latency_ms"));
    assert!(obj.contains_key("retry_count"));
}

#[tokio::test]
async fn api_call_defaults_to_ios_when_platform_absent() {
    let app = setup().await;
    let (_uid, token) = create_test_user(&app).await;

    let body = json!({
        "events": [{
            "type": "api_call",
            "payload": {"endpoint": "/health", "method": "GET", "status": 200}
        }]
    });

    let resp = app
        .app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/v1/telemetry/report",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let rows = wait_for_events(&app.pool, "api_call", 1).await;
    assert_eq!(rows[0].0, "ios");
}

#[tokio::test]
async fn api_call_disallowed_fields_are_scrubbed() {
    let app = setup().await;
    let (_uid, token) = create_test_user(&app).await;

    let body = json!({
        "events": [{
            "type": "api_call",
            "platform": "web",
            "payload": {
                "endpoint": "/account",
                "method": "GET",
                "status": 200,
                // Disallowed — must never be persisted.
                "request_body": {"password": "hunter2"},
                "response_body": "secret-data",
                "user_id": "1f2e3d4c",
                "authorization": "Bearer abc"
            }
        }]
    });

    let resp = app
        .app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/v1/telemetry/report",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json: Value = body_json(resp).await;
    assert_eq!(json["accepted"], 1);

    wait_for_events(&app.pool, "api_call", 1).await;
    let stored: (Value,) =
        sqlx::query_as("SELECT payload FROM app_events WHERE event_type = 'api_call' LIMIT 1")
            .fetch_one(&app.pool)
            .await
            .unwrap();
    let obj = stored.0.as_object().unwrap();
    assert!(!obj.contains_key("request_body"));
    assert!(!obj.contains_key("response_body"));
    assert!(!obj.contains_key("user_id"));
    assert!(!obj.contains_key("authorization"));
    // Allowlisted fields survive.
    assert_eq!(
        obj.get("endpoint").and_then(|v| v.as_str()),
        Some("/account")
    );
}

#[tokio::test]
async fn api_call_with_health_keyword_is_rejected_and_not_stored() {
    let app = setup().await;
    let (_uid, token) = create_test_user(&app).await;

    let body = json!({
        "events": [{
            "type": "api_call",
            "platform": "web",
            "payload": {
                "endpoint": "/glucose",
                "method": "GET",
                "status": 200
            }
        }]
    });

    let resp = app
        .app
        .clone()
        .oneshot(auth_request(
            "POST",
            "/api/v1/telemetry/report",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json: Value = body_json(resp).await;
    assert_eq!(json["accepted"], 0);
    assert_eq!(json["rejected"], 1);

    let count: (i64,) =
        sqlx::query_as("SELECT count(*) FROM app_events WHERE event_type = 'api_call'")
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(count.0, 0);
}

/// Migration 0029 adds a CHECK constraint restricting platform to the known
/// set. Existing 'ios' data must remain insertable, 'web' must now be accepted,
/// and anything else must be rejected by the constraint.
#[tokio::test]
async fn platform_check_constraint_allows_ios_and_web_rejects_others() {
    let app = setup().await;

    // Existing-style 'ios' row (what all pre-migration rows looked like).
    sqlx::query(
        "INSERT INTO app_events (event_type, payload, platform) VALUES ('crash', '{}', 'ios')",
    )
    .execute(&app.pool)
    .await
    .expect("ios insert should succeed");

    // New 'web' row.
    sqlx::query(
        "INSERT INTO app_events (event_type, payload, platform) VALUES ('api_call', '{}', 'web')",
    )
    .execute(&app.pool)
    .await
    .expect("web insert should succeed");

    // Default (NULL via column default 'ios') still works.
    sqlx::query("INSERT INTO app_events (event_type, payload) VALUES ('screen', '{}')")
        .execute(&app.pool)
        .await
        .expect("default-platform insert should succeed");

    // Unknown platform must be rejected by the CHECK constraint.
    let err = sqlx::query(
        "INSERT INTO app_events (event_type, payload, platform) VALUES ('crash', '{}', 'android')",
    )
    .execute(&app.pool)
    .await
    .expect_err("android insert should be rejected by check constraint");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("app_events_platform_check") || msg.contains("check"),
        "expected check constraint violation, got: {msg}"
    );

    // The two valid platform values are preserved.
    let platforms: Vec<(String,)> =
        sqlx::query_as("SELECT platform FROM app_events ORDER BY platform")
            .fetch_all(&app.pool)
            .await
            .unwrap();
    let values: Vec<&str> = platforms.iter().map(|p| p.0.as_str()).collect();
    assert_eq!(values, vec!["ios", "ios", "web"]);
}

#[tokio::test]
async fn telemetry_requires_authentication() {
    let app = setup().await;

    let body = json!({"events": [{"type": "api_call", "payload": {"endpoint": "/x"}}]});
    let req = http::Request::builder()
        .method("POST")
        .uri("/api/v1/telemetry/report")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
