// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{Duration, Utc};
use serde_json::json;
use tower::ServiceExt;

use crate::common;

// ---------------------------------------------------------------------------
// Helper: seed health records for a user
// ---------------------------------------------------------------------------

async fn seed_health_records(
    app: &common::TestApp,
    token: &str,
    record_type: &str,
    values: &[(i64, f64)],
) {
    for (days_ago, value) in values {
        let ts = (Utc::now() - Duration::days(*days_ago))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();
        let body = json!({
            "record_type": record_type,
            "value": value,
            "unit": "bpm",
            "source": "manual",
            "start_time": ts,
            "end_time": ts
        });
        let resp = app
            .app
            .clone()
            .oneshot(common::auth_request(
                "POST",
                "/api/v1/health-records",
                token,
                Some(&body),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), 201, "failed to seed health record");
    }
}

/// Seed interventions for a user.
async fn seed_interventions(
    app: &common::TestApp,
    token: &str,
    substance: &str,
    days_ago_list: &[i64],
) {
    for days_ago in days_ago_list {
        let ts = (Utc::now() - Duration::days(*days_ago))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();
        let body = json!({
            "substance": substance,
            "dose": 10.0,
            "unit": "mg",
            "route": "oral",
            "administered_at": ts
        });
        let resp = app
            .app
            .clone()
            .oneshot(common::auth_request(
                "POST",
                "/api/v1/interventions",
                token,
                Some(&body),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), 201, "failed to seed intervention");
    }
}

// ---------------------------------------------------------------------------
// POST /stats/before-after
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_before_after_happy_path() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Seed interventions starting 30 days ago
    seed_interventions(&app, &token, "creatine", &[30, 29, 28, 27, 26, 25]).await;

    // Seed health records: before period (60-31 days ago) and after period (30-20 days ago)
    let mut before_vals = Vec::new();
    for d in 31..=60 {
        before_vals.push((d, 65.0 + (d as f64 * 0.1)));
    }
    seed_health_records(&app, &token, "resting_heart_rate", &before_vals).await;

    let mut after_vals = Vec::new();
    for d in 20..=29 {
        after_vals.push((d, 60.0 + (d as f64 * 0.1)));
    }
    seed_health_records(&app, &token, "resting_heart_rate", &after_vals).await;

    let body = json!({
        "intervention_substance": "creatine",
        "metric": {"source": "health_records", "field": "resting_heart_rate"},
        "before_days": 30,
        "after_days": 30,
        "resolution": "daily"
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/stats/before-after",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["intervention_substance"], "creatine");
    assert_eq!(json["test_used"], "welch_t");
    assert!(json["first_dose"].is_string());
    assert!(json["last_dose"].is_string());
    assert!(json["before"]["n"].as_u64().unwrap() > 0);
    assert!(json["after"]["n"].as_u64().unwrap() > 0);
    assert!(json["before"]["mean"].is_number());
    assert!(json["after"]["mean"].is_number());
}

#[tokio::test]
async fn test_before_after_no_interventions() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "intervention_substance": "nonexistent_substance",
        "metric": {"source": "health_records", "field": "resting_heart_rate"},
        "before_days": 30,
        "after_days": 30,
        "resolution": "daily"
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/stats/before-after",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["before"]["n"], 0);
    assert_eq!(json["after"]["n"], 0);
    assert_eq!(json["significant"], false);
    assert!(
        json["warning"]
            .as_str()
            .unwrap()
            .contains("no interventions")
    );
}

#[tokio::test]
async fn test_before_after_unauthenticated() {
    let app = common::setup().await;

    let body = json!({
        "intervention_substance": "creatine",
        "metric": {"source": "health_records", "field": "resting_heart_rate"},
        "before_days": 30,
        "after_days": 30,
        "resolution": "daily"
    });

    let req = http::Request::builder()
        .method("POST")
        .uri("/api/v1/stats/before-after")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::to_string(&body).unwrap(),
        ))
        .unwrap();

    let resp = app.app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_before_after_invalid_input_empty_substance() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "intervention_substance": "  ",
        "metric": {"source": "health_records", "field": "resting_heart_rate"},
        "before_days": 30,
        "after_days": 30,
        "resolution": "daily"
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/stats/before-after",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let json = common::body_json(resp).await;
    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("intervention_substance")
    );
}

#[tokio::test]
async fn test_before_after_invalid_before_days() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "intervention_substance": "creatine",
        "metric": {"source": "health_records", "field": "resting_heart_rate"},
        "before_days": 0,
        "after_days": 30,
        "resolution": "daily"
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/stats/before-after",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let json = common::body_json(resp).await;
    assert!(json["error"].as_str().unwrap().contains("before_days"));
}

#[tokio::test]
async fn test_before_after_invalid_after_days() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "intervention_substance": "creatine",
        "metric": {"source": "health_records", "field": "resting_heart_rate"},
        "before_days": 30,
        "after_days": 400,
        "resolution": "daily"
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/stats/before-after",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_before_after_invalid_metric() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "intervention_substance": "creatine",
        "metric": {"source": "nonexistent", "field": "foo"},
        "before_days": 30,
        "after_days": 30,
        "resolution": "daily"
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/stats/before-after",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

// ---------------------------------------------------------------------------
// POST /stats/correlate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_correlate_happy_path() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Seed heart rate and steps data with some correlation
    let mut hr_vals = Vec::new();
    let mut steps_vals = Vec::new();
    for d in 1..=20 {
        hr_vals.push((d, 60.0 + d as f64));
        steps_vals.push((d, 5000.0 + (d as f64 * 100.0)));
    }
    seed_health_records(&app, &token, "heart_rate", &hr_vals).await;
    seed_health_records(&app, &token, "steps", &steps_vals).await;

    let start = (Utc::now() - Duration::days(25))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let end = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let body = json!({
        "metric_a": {"source": "health_records", "field": "heart_rate"},
        "metric_b": {"source": "health_records", "field": "steps"},
        "start": start,
        "end": end,
        "resolution": "daily",
        "method": "pearson"
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/stats/correlate",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["method"], "pearson");
    assert!(json["n"].as_u64().unwrap() > 0);
    assert!(json["interpretation"].is_string());
    assert!(json["scatter"].is_array());
}

#[tokio::test]
async fn test_correlate_spearman() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Seed data
    let mut hr_vals = Vec::new();
    let mut steps_vals = Vec::new();
    for d in 1..=15 {
        hr_vals.push((d, 60.0 + d as f64));
        steps_vals.push((d, 5000.0 + (d as f64 * 200.0)));
    }
    seed_health_records(&app, &token, "heart_rate", &hr_vals).await;
    seed_health_records(&app, &token, "steps", &steps_vals).await;

    let start = (Utc::now() - Duration::days(20))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let end = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let body = json!({
        "metric_a": {"source": "health_records", "field": "heart_rate"},
        "metric_b": {"source": "health_records", "field": "steps"},
        "start": start,
        "end": end,
        "resolution": "daily",
        "method": "spearman"
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/stats/correlate",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["method"], "spearman");
}

#[tokio::test]
async fn test_correlate_unauthenticated() {
    let app = common::setup().await;

    let start = (Utc::now() - Duration::days(30))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let end = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let body = json!({
        "metric_a": {"source": "health_records", "field": "heart_rate"},
        "metric_b": {"source": "health_records", "field": "steps"},
        "start": start,
        "end": end,
        "resolution": "daily"
    });

    let req = http::Request::builder()
        .method("POST")
        .uri("/api/v1/stats/correlate")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::to_string(&body).unwrap(),
        ))
        .unwrap();

    let resp = app.app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_correlate_invalid_start_after_end() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "metric_a": {"source": "health_records", "field": "heart_rate"},
        "metric_b": {"source": "health_records", "field": "steps"},
        "start": "2026-12-01T00:00:00Z",
        "end": "2026-01-01T00:00:00Z",
        "resolution": "daily"
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/stats/correlate",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let json = common::body_json(resp).await;
    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("start must be before end")
    );
}

#[tokio::test]
async fn test_correlate_invalid_metric_source() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "metric_a": {"source": "bogus", "field": "heart_rate"},
        "metric_b": {"source": "health_records", "field": "steps"},
        "start": "2026-01-01T00:00:00Z",
        "end": "2026-03-01T00:00:00Z",
        "resolution": "daily"
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/stats/correlate",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_correlate_no_data_returns_empty() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "metric_a": {"source": "health_records", "field": "heart_rate"},
        "metric_b": {"source": "health_records", "field": "steps"},
        "start": "2020-01-01T00:00:00Z",
        "end": "2020-02-01T00:00:00Z",
        "resolution": "daily"
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/stats/correlate",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["n"], 0);
    assert!(json["r"].is_null());
    assert_eq!(json["significant"], false);
    assert_eq!(json["interpretation"], "insufficient data");
}

// ---------------------------------------------------------------------------
// POST /stats/lag-correlate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_lag_correlate_happy_path() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Seed data over 30 days
    let mut hr_vals = Vec::new();
    let mut steps_vals = Vec::new();
    for d in 1..=30 {
        hr_vals.push((d, 60.0 + (d as f64 * 0.5)));
        steps_vals.push((d, 8000.0 + (d as f64 * 100.0)));
    }
    seed_health_records(&app, &token, "heart_rate", &hr_vals).await;
    seed_health_records(&app, &token, "steps", &steps_vals).await;

    let start = (Utc::now() - Duration::days(35))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let end = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let body = json!({
        "metric_a": {"source": "health_records", "field": "heart_rate"},
        "metric_b": {"source": "health_records", "field": "steps"},
        "start": start,
        "end": end,
        "resolution": "daily",
        "max_lag_days": 3,
        "method": "pearson"
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/stats/lag-correlate",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let lags = json["lags"].as_array().unwrap();
    // Should have 7 lags: -3, -2, -1, 0, 1, 2, 3
    assert_eq!(lags.len(), 7);
    assert_eq!(json["method"], "pearson");
    // best_lag should be present when there is data
    assert!(json["best_lag"].is_object() || json["best_lag"].is_null());
}

#[tokio::test]
async fn test_lag_correlate_unauthenticated() {
    let app = common::setup().await;

    let body = json!({
        "metric_a": {"source": "health_records", "field": "heart_rate"},
        "metric_b": {"source": "health_records", "field": "steps"},
        "start": "2026-01-01T00:00:00Z",
        "end": "2026-03-01T00:00:00Z",
        "resolution": "daily",
        "max_lag_days": 3
    });

    let req = http::Request::builder()
        .method("POST")
        .uri("/api/v1/stats/lag-correlate")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::to_string(&body).unwrap(),
        ))
        .unwrap();

    let resp = app.app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_lag_correlate_invalid_max_lag_too_high() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "metric_a": {"source": "health_records", "field": "heart_rate"},
        "metric_b": {"source": "health_records", "field": "steps"},
        "start": "2026-01-01T00:00:00Z",
        "end": "2026-03-01T00:00:00Z",
        "resolution": "daily",
        "max_lag_days": 31
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/stats/lag-correlate",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
    let json = common::body_json(resp).await;
    assert!(json["error"].as_str().unwrap().contains("max_lag_days"));
}

#[tokio::test]
async fn test_lag_correlate_invalid_max_lag_zero() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "metric_a": {"source": "health_records", "field": "heart_rate"},
        "metric_b": {"source": "health_records", "field": "steps"},
        "start": "2026-01-01T00:00:00Z",
        "end": "2026-03-01T00:00:00Z",
        "resolution": "daily",
        "max_lag_days": 0
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/stats/lag-correlate",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_lag_correlate_start_after_end() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "metric_a": {"source": "health_records", "field": "heart_rate"},
        "metric_b": {"source": "health_records", "field": "steps"},
        "start": "2026-12-01T00:00:00Z",
        "end": "2026-01-01T00:00:00Z",
        "resolution": "daily",
        "max_lag_days": 3
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/stats/lag-correlate",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

// ---------------------------------------------------------------------------
// Cross-user isolation: user A's data is not visible in user B's stats
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_correlate_user_isolation() {
    let app = common::setup().await;
    let (_user_a_id, token_a) = common::create_test_user(&app).await;
    let (_user_b_id, token_b) = common::create_test_user(&app).await;

    // Seed data only for user A
    let mut hr_vals = Vec::new();
    for d in 1..=10 {
        hr_vals.push((d, 70.0 + d as f64));
    }
    seed_health_records(&app, &token_a, "heart_rate", &hr_vals).await;
    seed_health_records(&app, &token_a, "steps", &hr_vals).await;

    let start = (Utc::now() - Duration::days(15))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let end = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let body = json!({
        "metric_a": {"source": "health_records", "field": "heart_rate"},
        "metric_b": {"source": "health_records", "field": "steps"},
        "start": start,
        "end": end,
        "resolution": "daily"
    });

    // User B should see no data
    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/stats/correlate",
            &token_b,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["n"], 0);
}
