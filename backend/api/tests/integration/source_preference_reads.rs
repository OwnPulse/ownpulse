// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Verifies `source_preferences` is applied at read time for default
//! aggregate reads (explore/series, dashboard summary), while `GET
//! /health-records` and export paths stay raw. See
//! `db::health_records::SOURCE_PREFERENCE_EXCLUSION`.
//!
//! Every dedup pair must collapse to exactly one visible row in aggregate
//! reads, in all four cases: no preference at all (defaults to the
//! original/first-arriving row), a preference naming the original's source,
//! a preference naming the later-arriving row's source (regardless of
//! arrival order), and a preference naming a source absent from the pair
//! (a no-op — falls back to the default).

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

/// Insert `first_source`/`first_value` then `second_source`/`second_value`
/// within the 60s/2% dedup window, so the second lands with `duplicate_of`
/// pointing at the first (the "original"). Whichever source arrives first
/// becomes the default-canonical row absent a preference.
async fn seed_duplicate_pair(
    app: &common::TestApp,
    token: &str,
    record_type: &str,
    first_source: &str,
    first_value: f64,
    second_source: &str,
    second_value: f64,
) {
    insert_record(
        app,
        token,
        first_source,
        record_type,
        first_value,
        &recent_ts(10),
    )
    .await;
    insert_record(
        app,
        token,
        second_source,
        record_type,
        second_value,
        &recent_ts(10),
    )
    .await;
}

async fn series_points(app: &common::TestApp, token: &str, field: &str) -> Vec<serde_json::Value> {
    let start = (Utc::now() - Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let end = (Utc::now() + Duration::hours(1)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!(
                "/api/v1/explore/series?source=health_records&field={field}&start={start}&end={end}&resolution=daily"
            ),
            token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    json["points"].as_array().unwrap().clone()
}

#[tokio::test]
async fn test_series_excludes_later_row_when_preference_names_original() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // garmin arrives first (original), oura is the later duplicate.
    seed_duplicate_pair(&app, &token, "heart_rate", "garmin", 60.0, "oura", 60.5).await;
    set_preference(&app, &token, "heart_rate", "garmin").await;

    let points = series_points(&app, &token, "heart_rate").await;
    assert_eq!(points.len(), 1, "one daily bucket");
    assert_eq!(
        points[0]["n"], 1,
        "exactly one row must survive the collapse"
    );
    assert_eq!(points[0]["v"], 60.0, "the preferred (original) row's value");
}

#[tokio::test]
async fn test_series_reverse_arrival_order_still_honors_preference() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // oura arrives first (original, duplicate_of IS NULL), garmin is the
    // later row. Preferring garmin must still surface garmin's value even
    // though garmin is NOT the row `duplicate_of` was stamped on — this is
    // exactly the ordering-dependent bug a naive
    // `duplicate_of IS NOT NULL AND source <> preferred` check gets wrong.
    seed_duplicate_pair(&app, &token, "heart_rate", "oura", 65.0, "garmin", 65.5).await;
    set_preference(&app, &token, "heart_rate", "garmin").await;

    let points = series_points(&app, &token, "heart_rate").await;
    assert_eq!(points.len(), 1);
    assert_eq!(
        points[0]["n"], 1,
        "exactly one row must survive the collapse"
    );
    assert_eq!(
        points[0]["v"], 65.5,
        "garmin (the later-arriving, non-`duplicate_of`-partner row) must win"
    );
}

#[tokio::test]
async fn test_series_collapses_to_original_without_any_preference() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // No source_preferences row at all — every user's default state. The
    // pair must still collapse to exactly one row (the original / first
    // arrival), not double-count both.
    seed_duplicate_pair(&app, &token, "heart_rate", "garmin", 60.0, "oura", 60.5).await;

    let points = series_points(&app, &token, "heart_rate").await;
    assert_eq!(points.len(), 1);
    assert_eq!(
        points[0]["n"], 1,
        "absent a preference, the pair must still collapse to one row"
    );
    assert_eq!(
        points[0]["v"], 60.0,
        "default canonical row is the original (first-arriving) one"
    );
}

#[tokio::test]
async fn test_series_preference_naming_source_absent_from_pair_is_a_noop() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // garmin/oura pair, but the preference names a third source that isn't
    // part of this pair at all (stale/typo). Must fall back to the default
    // (original wins) rather than hiding data with nothing to replace it.
    seed_duplicate_pair(&app, &token, "heart_rate", "garmin", 70.0, "oura", 70.5).await;
    set_preference(&app, &token, "heart_rate", "healthkit").await;

    let points = series_points(&app, &token, "heart_rate").await;
    assert_eq!(points.len(), 1);
    assert_eq!(
        points[0]["n"], 1,
        "a preference naming an absent source must not vanish the whole pair"
    );
    assert_eq!(
        points[0]["v"], 70.0,
        "falls back to the original row when the preference is a no-op"
    );
}

#[tokio::test]
async fn test_series_preference_scoped_to_its_own_metric() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Preference set for heart_rate...
    seed_duplicate_pair(&app, &token, "heart_rate", "garmin", 60.0, "oura", 60.5).await;
    set_preference(&app, &token, "heart_rate", "garmin").await;

    // ...must not affect a different metric's duplicate pair, which still
    // collapses to its own default (original) row.
    seed_duplicate_pair(
        &app,
        &token,
        "respiratory_rate",
        "garmin",
        14.0,
        "oura",
        14.1,
    )
    .await;

    let points = series_points(&app, &token, "respiratory_rate").await;
    assert_eq!(points.len(), 1);
    assert_eq!(
        points[0]["n"], 1,
        "respiratory_rate has no preference of its own, but still collapses to one row"
    );
    assert_eq!(points[0]["v"], 14.0);
}

#[tokio::test]
async fn test_sum_aggregation_collapses_duplicate_pair_without_preference() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // `steps` uses Sum aggregation. Without this collapse, two sources
    // reporting overlapping step counts would silently double the daily
    // total — a count-only assertion wouldn't catch this if the wrong row
    // were the one hidden, so this asserts the summed value too.
    seed_duplicate_pair(&app, &token, "steps", "garmin", 1000.0, "oura", 1010.0).await;

    let points = series_points(&app, &token, "steps").await;
    assert_eq!(points.len(), 1);
    assert_eq!(points[0]["n"], 1);
    assert_eq!(
        points[0]["v"], 1000.0,
        "sum must reflect only the canonical (original) row, not both sources added together"
    );
}

#[tokio::test]
async fn test_dashboard_summary_excludes_deduped_row_when_preference_set() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    seed_duplicate_pair(&app, &token, "heart_rate", "garmin", 60.0, "oura", 60.5).await;
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
async fn test_dashboard_summary_collapses_duplicate_pair_by_default() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // No preference at all — the 7-day count must still collapse the pair.
    seed_duplicate_pair(&app, &token, "heart_rate", "garmin", 60.0, "oura", 60.5).await;

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
        "absent a preference, the pair must still collapse to one row"
    );
}

#[tokio::test]
async fn test_health_records_list_stays_raw_regardless_of_preference() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    seed_duplicate_pair(&app, &token, "heart_rate", "garmin", 60.0, "oura", 60.5).await;
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

    seed_duplicate_pair(&app, &token, "heart_rate", "garmin", 60.0, "oura", 60.5).await;
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

// Diagnostic-only: prints the query plan for SOURCE_PREFERENCE_EXCLUSION so
// its index usage can be inspected by hand. `#[ignore]`d — not part of the
// normal suite, run explicitly with `cargo test -- --ignored --nocapture`.
#[tokio::test]
#[ignore]
async fn debug_explain_source_preference_exclusion() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    seed_duplicate_pair(&app, &token, "heart_rate", "garmin", 60.0, "oura", 60.5).await;
    set_preference(&app, &token, "heart_rate", "garmin").await;

    let sql = format!(
        "EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT)
         SELECT date_trunc('day', hr.start_time) AS bucket,
                AVG(hr.value) AS avg_val,
                COUNT(*) AS cnt
         FROM health_records hr
         WHERE hr.user_id = $1 AND hr.record_type = $2
           AND hr.start_time >= now() - interval '1 hour'
           AND hr.start_time <= now() + interval '1 hour'
           AND hr.value IS NOT NULL
           AND {}
         GROUP BY bucket
         ORDER BY bucket ASC",
        api::db::health_records::SOURCE_PREFERENCE_EXCLUSION
    );

    let rows: Vec<(String,)> = sqlx::query_as(&sql)
        .bind(user_id)
        .bind("heart_rate")
        .fetch_all(&app.pool)
        .await
        .unwrap();

    for (line,) in rows {
        eprintln!("{line}");
    }
}
