// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Verifies `source_preferences` is applied at read time for default
//! aggregate reads (explore/series, dashboard summary), while `GET
//! /health-records` and export paths stay raw. See
//! `db::health_records::SOURCE_PREFERENCE_EXCLUSION`.

use chrono::{Duration, SecondsFormat, Utc};
use serde_json::json;
use tower::ServiceExt;

use crate::common;

/// RFC3339 timestamp `minutes_ago` in the past.
fn recent_ts(minutes_ago: i64) -> String {
    (Utc::now() - Duration::minutes(minutes_ago)).to_rfc3339_opts(SecondsFormat::Secs, true)
}

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

async fn set_preference(app: &common::TestApp, token: &str, metric_type: &str, preferred: &str) {
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/source-preferences",
            token,
            Some(&json!({
                "metric_type": metric_type,
                "preferred_source": preferred,
            })),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "failed to set source preference");
}

/// Insert a garmin record, then an oura record within the 60s/2% dedup
/// window so it lands with `duplicate_of` pointing at the garmin row.
async fn seed_duplicate_pair(app: &common::TestApp, token: &str, record_type: &str) {
    insert_record(app, token, "garmin", record_type, 60.0, &recent_ts(10)).await;
    insert_record(app, token, "oura", record_type, 60.5, &recent_ts(10)).await;
}

#[tokio::test]
async fn test_series_excludes_deduped_row_when_preference_set() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    seed_duplicate_pair(&app, &token, "heart_rate").await;
    set_preference(&app, &token, "heart_rate", "garmin").await;

    let start = (Utc::now() - Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let end = (Utc::now() + Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            &format!(
                "/api/v1/explore/series?source=health_records&field=heart_rate&start={start}&end={end}&resolution=daily"
            ),
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let points = json["points"].as_array().unwrap();
    assert_eq!(points.len(), 1, "one daily bucket");
    // Only the preferred (garmin) row should count toward the aggregate.
    assert_eq!(points[0]["n"], 1);
    assert_eq!(points[0]["v"], 60.0);
}

#[tokio::test]
async fn test_series_includes_both_rows_without_preference() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Same duplicate pair, but no source_preferences row is set for this metric.
    seed_duplicate_pair(&app, &token, "heart_rate").await;

    let start = (Utc::now() - Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let end = (Utc::now() + Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            &format!(
                "/api/v1/explore/series?source=health_records&field=heart_rate&start={start}&end={end}&resolution=daily"
            ),
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let points = json["points"].as_array().unwrap();
    assert_eq!(points.len(), 1);
    // Current (unchanged) behavior: both rows counted absent a preference.
    assert_eq!(points[0]["n"], 2);
}

#[tokio::test]
async fn test_series_preference_scoped_to_its_own_metric() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Preference set for heart_rate...
    seed_duplicate_pair(&app, &token, "heart_rate").await;
    set_preference(&app, &token, "heart_rate", "garmin").await;

    // ...must not affect a different metric's duplicate pair.
    seed_duplicate_pair(&app, &token, "respiratory_rate").await;

    let start = (Utc::now() - Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let end = (Utc::now() + Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            &format!(
                "/api/v1/explore/series?source=health_records&field=respiratory_rate&start={start}&end={end}&resolution=daily"
            ),
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let points = json["points"].as_array().unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(
        points[0]["n"], 2,
        "respiratory_rate has no preference of its own, both rows count"
    );
}

#[tokio::test]
async fn test_dashboard_summary_excludes_deduped_row_when_preference_set() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    seed_duplicate_pair(&app, &token, "heart_rate").await;
    set_preference(&app, &token, "heart_rate", "garmin").await;

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/dashboard/summary",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(
        json["health_record_count_7d"], 1,
        "the deduped oura row must not be double-counted"
    );
}

#[tokio::test]
async fn test_health_records_list_stays_raw_regardless_of_preference() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    seed_duplicate_pair(&app, &token, "heart_rate").await;
    set_preference(&app, &token, "heart_rate", "garmin").await;

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/health-records?record_type=heart_rate",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let rows = json.as_array().unwrap();
    assert_eq!(
        rows.len(),
        2,
        "GET /health-records must preserve provenance for both sources"
    );
}

#[tokio::test]
async fn test_export_json_stays_raw_regardless_of_preference() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    seed_duplicate_pair(&app, &token, "heart_rate").await;
    set_preference(&app, &token, "heart_rate", "garmin").await;

    let resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/export/json",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let hr = json["health_records"].as_array().unwrap();
    let heart_rate_rows: Vec<_> = hr
        .iter()
        .filter(|r| r["record_type"] == "heart_rate")
        .collect();
    assert_eq!(
        heart_rate_rows.len(),
        2,
        "export must never drop a record for provenance reasons"
    );
}
