// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;

use crate::common;

/// Insert a health record for the test user directly via the API.
async fn insert_record(
    app: &common::TestApp,
    token: &str,
    source: &str,
    record_type: &str,
    value: f64,
    start_time: &str,
) {
    let body = json!({
        "source": source,
        "record_type": record_type,
        "value": value,
        "unit": "bpm",
        "start_time": start_time,
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
    assert_eq!(resp.status(), 201, "failed to insert health record");
}

#[tokio::test]
async fn test_overlap_scan_reports_metrics_with_multiple_sources() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // heart_rate has two sources -> should be reported.
    insert_record(&app, &token, "garmin", "heart_rate", 60.0, "2026-05-20T10:00:00Z").await;
    insert_record(&app, &token, "garmin", "heart_rate", 61.0, "2026-05-20T10:01:00Z").await;
    insert_record(&app, &token, "oura", "heart_rate", 62.0, "2026-05-20T10:02:00Z").await;

    // weight has a single source -> should NOT be reported.
    insert_record(&app, &token, "manual", "weight", 80.0, "2026-05-20T08:00:00Z").await;

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/sources/overlap-scan",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = common::body_json(resp).await;

    let metrics = body["metrics"].as_array().expect("metrics array");
    assert_eq!(metrics.len(), 1, "only heart_rate overlaps");

    let metric = &metrics[0];
    assert_eq!(metric["metric_type"], "heart_rate");

    let sources = metric["sources"].as_array().expect("sources array");
    assert_eq!(sources.len(), 2);
    // garmin has 2 records, oura has 1 -> garmin first (desc by count).
    assert_eq!(sources[0]["source"], "garmin");
    assert_eq!(sources[0]["record_count"], 2);
    assert_eq!(sources[1]["source"], "oura");
    assert_eq!(sources[1]["record_count"], 1);
}

#[tokio::test]
async fn test_overlap_scan_empty_when_no_overlap() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Single-source metric only.
    insert_record(&app, &token, "manual", "weight", 80.0, "2026-05-20T08:00:00Z").await;

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/sources/overlap-scan",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = common::body_json(resp).await;
    assert!(body["metrics"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_overlap_scan_requires_auth() {
    let app = common::setup().await;

    let resp = app
        .app
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri("/api/v1/sources/overlap-scan")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_overlap_scan_is_user_scoped() {
    let app = common::setup().await;
    let (_user_a, token_a) = common::create_test_user(&app).await;

    // User A has overlapping heart_rate from two sources.
    insert_record(&app, &token_a, "garmin", "heart_rate", 60.0, "2026-05-20T10:00:00Z").await;
    insert_record(&app, &token_a, "oura", "heart_rate", 62.0, "2026-05-20T10:02:00Z").await;

    // A second user with no data must see an empty scan.
    let (_user_b, token_b) = common::create_test_user(&app).await;
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/sources/overlap-scan",
            &token_b,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = common::body_json(resp).await;
    assert!(
        body["metrics"].as_array().unwrap().is_empty(),
        "user B must not see user A's overlaps"
    );
}
