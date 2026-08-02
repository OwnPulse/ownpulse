// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Integration tests for the protocol adherence / missed-doses / run-doses
//! read endpoints:
//!   - GET /protocols/runs/:run_id/doses
//!   - GET /protocols/runs/missed-doses
//!   - GET /protocols/runs/:run_id/adherence
//!   - RunResponse.adherence_pct / doses_missed (list_active_runs + create_run)

use chrono::{Duration, Utc};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::common;

/// Create a two-line protocol recipe:
///   - line A: scheduled every day for `duration_days`
///   - line B: scheduled every OTHER day (0, 2, 4, ...) for `duration_days`
async fn create_two_line_recipe(app: &common::TestApp, token: &str, duration_days: i32) -> Value {
    let daily: Vec<bool> = vec![true; duration_days as usize];
    let every_other: Vec<bool> = (0..duration_days).map(|d| d % 2 == 0).collect();

    let body = json!({
        "name": "Adherence Test Stack",
        "duration_days": duration_days,
        "lines": [
            {
                "substance": "BPC-157",
                "dose": 250.0,
                "unit": "mcg",
                "schedule_pattern": daily,
                "sort_order": 0
            },
            {
                "substance": "TB-500",
                "dose": 2.0,
                "unit": "mg",
                "schedule_pattern": every_other,
                "sort_order": 1
            }
        ]
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/protocols",
            token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    common::body_json(resp).await
}

/// Start a run on `protocol_id` backdated so `start_date = today - days_ago`.
async fn start_run(app: &common::TestApp, token: &str, protocol_id: &str, days_ago: i64) -> Value {
    let start_date = (Utc::now() - Duration::days(days_ago)).date_naive();
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/runs"),
            token,
            Some(&json!({"start_date": start_date.to_string()})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    common::body_json(resp).await
}

async fn log_dose(
    app: &common::TestApp,
    token: &str,
    run_id: &str,
    line_id: &str,
    day_number: i32,
) {
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/log"),
            token,
            Some(&json!({"protocol_line_id": line_id, "day_number": day_number})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "log_dose failed for day {day_number}");
}

async fn skip_dose(
    app: &common::TestApp,
    token: &str,
    run_id: &str,
    line_id: &str,
    day_number: i32,
) {
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/skip"),
            token,
            Some(&json!({"protocol_line_id": line_id, "day_number": day_number})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204, "skip_dose failed for day {day_number}");
}

/// Seed the canonical mid-run scenario used by several tests:
///
/// A run started 5 days ago (`today_day == 5`), duration 10 days.
/// Line A (daily): day0 completed, day1 skipped, day2/day3 missed (no dose
/// logged), day4 completed, day5 (today) left pending.
/// Line B (every other day: 0,2,4,6,8): day0 completed, day2 skipped, day4
/// missed (no dose), day6/day8 not yet reached (in the future).
async fn seed_mid_run(app: &common::TestApp, token: &str) -> (String, String, String) {
    let protocol = create_two_line_recipe(app, token, 10).await;
    let protocol_id = protocol["id"].as_str().unwrap().to_string();
    let line_a = protocol["lines"][0]["id"].as_str().unwrap().to_string();
    let line_b = protocol["lines"][1]["id"].as_str().unwrap().to_string();

    let run = start_run(app, token, &protocol_id, 5).await;
    let run_id = run["id"].as_str().unwrap().to_string();

    log_dose(app, token, &run_id, &line_a, 0).await;
    skip_dose(app, token, &run_id, &line_a, 1).await;
    // day2, day3 left unlogged -> missed
    log_dose(app, token, &run_id, &line_a, 4).await;
    // day5 (today) left unlogged -> pending

    log_dose(app, token, &run_id, &line_b, 0).await;
    skip_dose(app, token, &run_id, &line_b, 2).await;
    // day4 left unlogged -> missed (line B is scheduled true on day4)

    (protocol_id, run_id, line_a)
}

fn find_line<'a>(items: &'a [Value], line_id: &str) -> &'a Value {
    items
        .iter()
        .find(|i| i["protocol_line_id"] == line_id)
        .unwrap_or_else(|| panic!("no adherence entry for line {line_id}"))
}

/// Create a single-line, daily-scheduled protocol recipe.
async fn create_one_line_recipe(app: &common::TestApp, token: &str, duration_days: i32) -> Value {
    let daily: Vec<bool> = vec![true; duration_days as usize];
    let body = json!({
        "name": "Single Line Stack",
        "duration_days": duration_days,
        "lines": [{
            "substance": "Vitamin D",
            "dose": 1000.0,
            "unit": "IU",
            "schedule_pattern": daily,
            "sort_order": 0
        }]
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/protocols",
            token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    common::body_json(resp).await
}

async fn fetch_doses(
    app: &common::TestApp,
    token: &str,
    run_id: &str,
    from_day: i32,
    to_day: i32,
) -> Vec<Value> {
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/runs/{run_id}/doses?from_day={from_day}&to_day={to_day}"),
            token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    common::body_json(resp).await.as_array().unwrap().clone()
}

async fn fetch_adherence(app: &common::TestApp, token: &str, run_id: &str) -> Value {
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/runs/{run_id}/adherence"),
            token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    common::body_json(resp).await
}

async fn fetch_active_runs(app: &common::TestApp, token: &str) -> Vec<Value> {
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols/runs/active",
            token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    common::body_json(resp).await.as_array().unwrap().clone()
}

/// Independently recompute closed-day adherence totals from `/doses`'
/// per-day statuses (a second implementation of the same rule, cross-checked
/// against `/adherence`'s own SQL-side computation). `today_day` mirrors
/// `dose_status::closed_bound`'s cutoff: a day counts only if
/// `day_number < today_day`.
fn derive_adherence_from_doses(doses: &[Value], today_day: i64) -> (i64, i64, i64, i64) {
    let mut scheduled = 0i64;
    let mut completed = 0i64;
    let mut skipped = 0i64;
    let mut missed = 0i64;
    for d in doses {
        let day = d["day_number"].as_i64().unwrap();
        if day >= today_day {
            continue;
        }
        scheduled += 1;
        match d["status"].as_str().unwrap() {
            "completed" => completed += 1,
            "skipped" => skipped += 1,
            "missed" => missed += 1,
            other => panic!("unexpected status {other} for a closed day"),
        }
    }
    (scheduled, completed, skipped, missed)
}

// ---------------------------------------------------------------------------
// GET /protocols/runs/:run_id/doses
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_run_doses_default_range_reports_every_status() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;
    let (_protocol_id, run_id, line_a) = seed_mid_run(&app, &token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/runs/{run_id}/doses"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let items = common::body_json(resp).await;
    let items = items.as_array().unwrap();

    // Line A: days 0-5, all scheduled (daily) => 6 entries.
    // Line B: days 0,2,4 scheduled within the 0..=5 default range => 3 entries.
    assert_eq!(items.len(), 9, "unexpected items: {items:#?}");

    let by_day_a = |d: i64| -> &Value {
        items
            .iter()
            .find(|i| i["protocol_line_id"] == line_a && i["day_number"] == d)
            .unwrap()
    };

    assert_eq!(by_day_a(0)["status"], "completed");
    assert!(by_day_a(0)["intervention_id"].as_str().is_some());
    assert_eq!(by_day_a(1)["status"], "skipped");
    assert_eq!(by_day_a(2)["status"], "missed");
    assert!(by_day_a(2)["dose_id"].is_null());
    assert_eq!(by_day_a(3)["status"], "missed");
    assert_eq!(by_day_a(4)["status"], "completed");
    assert_eq!(by_day_a(5)["status"], "pending");
    assert!(by_day_a(5)["dose_id"].is_null());

    // Line B's odd days (1,3,5) are not scheduled and must not appear at all.
    assert!(
        !items
            .iter()
            .any(|i| i["protocol_line_id"] != line_a && i["day_number"] == 1)
    );
}

#[tokio::test]
async fn test_run_doses_explicit_range() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;
    let (_protocol_id, run_id, line_a) = seed_mid_run(&app, &token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/runs/{run_id}/doses?from_day=0&to_day=1"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let items = common::body_json(resp).await;
    let items = items.as_array().unwrap();
    // Line A day0+day1, line B day0 (day1 not scheduled) = 3 entries.
    assert_eq!(items.len(), 3);
    assert!(
        items
            .iter()
            .any(|i| i["protocol_line_id"] == line_a && i["day_number"] == 0)
    );
}

#[tokio::test]
async fn test_run_doses_out_of_bounds_range_returns_400() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;
    let (_protocol_id, run_id, _line_a) = seed_mid_run(&app, &token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/runs/{run_id}/doses?from_day=0&to_day=999"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_run_doses_negative_from_day_returns_400() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;
    let (_protocol_id, run_id, _line_a) = seed_mid_run(&app, &token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/runs/{run_id}/doses?from_day=-1&to_day=2"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_run_doses_backwards_range_returns_400() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;
    let (_protocol_id, run_id, _line_a) = seed_mid_run(&app, &token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/runs/{run_id}/doses?from_day=5&to_day=1"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_run_doses_unauthenticated_returns_401() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;
    let (_protocol_id, run_id, _line_a) = seed_mid_run(&app, &token).await;

    let req = axum::http::Request::builder()
        .method("GET")
        .uri(format!("/api/v1/protocols/runs/{run_id}/doses"))
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_run_doses_missing_run_returns_404() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/runs/{}/doses", uuid::Uuid::new_v4()),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_run_doses_foreign_run_returns_404() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;
    let (_uid2, token2) = common::create_test_user(&app).await;
    let (_protocol_id, run_id, _line_a) = seed_mid_run(&app, &token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/runs/{run_id}/doses"),
            &token2,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// ---------------------------------------------------------------------------
// GET /protocols/runs/:run_id/adherence
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_adherence_full_matrix_including_per_line() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;
    let (_protocol_id, run_id, line_a) = seed_mid_run(&app, &token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/runs/{run_id}/adherence"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;

    assert_eq!(json["run_id"], run_id);
    // Adherence is computed over CLOSED days only (day_number < today_day
    // == 5), so day5 (today, unlogged on line A) is excluded entirely —
    // scheduled_so_far no longer includes it.
    // Line A: closed days 0-4, all scheduled (daily) -> scheduled_so_far=5,
    // completed=2 (day0,day4), skipped=1 (day1), missed=2 (day2,day3).
    // Line B: closed days 0-4, scheduled at 0,2,4 -> scheduled_so_far=3,
    // completed=1 (day0), skipped=1 (day2), missed=1 (day4).
    assert_eq!(json["scheduled_so_far"], 8);
    assert_eq!(json["completed"], 3);
    assert_eq!(json["skipped"], 2);
    assert_eq!(json["missed"], 3);
    // adherence_pct = completed / (scheduled_so_far - skipped) * 100
    //               = 3 / (8 - 2) * 100 = 50.0
    let pct = json["adherence_pct"].as_f64().unwrap();
    assert!((pct - 50.0).abs() < 1e-9);

    let lines = json["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 2);

    let a = find_line(lines, &line_a);
    assert_eq!(a["scheduled_so_far"], 5);
    assert_eq!(a["completed"], 2);
    assert_eq!(a["skipped"], 1);
    assert_eq!(a["missed"], 2);
    // 2 / (5 - 1) * 100 = 50.0
    let a_pct = a["adherence_pct"].as_f64().unwrap();
    assert!((a_pct - 50.0).abs() < 1e-9);

    let b = lines
        .iter()
        .find(|l| l["protocol_line_id"] != line_a)
        .unwrap();
    assert_eq!(b["scheduled_so_far"], 3);
    assert_eq!(b["completed"], 1);
    assert_eq!(b["skipped"], 1);
    assert_eq!(b["missed"], 1);
    // 1 / (3 - 1) * 100 = 50.0
    let b_pct = b["adherence_pct"].as_f64().unwrap();
    assert!((b_pct - 50.0).abs() < 1e-9);
}

#[tokio::test]
async fn test_adherence_zero_scheduled_is_null_pct() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    // A protocol with a line scheduled nowhere yet, run started today with
    // duration 1 and pattern [false] would be invalid at creation (every
    // day must be reachable) — instead, use a future-dated run: nothing is
    // scheduled "so far" because the run hasn't started.
    let protocol = create_two_line_recipe(&app, &token, 5).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    let future_start = (Utc::now() + Duration::days(3)).date_naive();
    let run_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/runs"),
            &token,
            Some(&json!({"start_date": future_start.to_string()})),
        ))
        .await
        .unwrap();
    assert_eq!(run_resp.status(), 201);
    let run = common::body_json(run_resp).await;
    let run_id = run["id"].as_str().unwrap();
    // create_run's RunResponse should also report null/zero for a future run.
    assert!(run["adherence_pct"].is_null());
    assert_eq!(run["doses_missed"], 0);

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/runs/{run_id}/adherence"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["scheduled_so_far"], 0);
    assert!(json["adherence_pct"].is_null());
    for line in json["lines"].as_array().unwrap() {
        assert!(line["adherence_pct"].is_null());
    }
}

#[tokio::test]
async fn test_adherence_unauthenticated_returns_401() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;
    let (_protocol_id, run_id, _line_a) = seed_mid_run(&app, &token).await;

    let req = axum::http::Request::builder()
        .method("GET")
        .uri(format!("/api/v1/protocols/runs/{run_id}/adherence"))
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_adherence_foreign_run_returns_404() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;
    let (_uid2, token2) = common::create_test_user(&app).await;
    let (_protocol_id, run_id, _line_a) = seed_mid_run(&app, &token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/runs/{run_id}/adherence"),
            &token2,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_adherence_missing_run_returns_404() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/runs/{}/adherence", uuid::Uuid::new_v4()),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

/// A second run of the *same* protocol must have its own, independent
/// adherence — exercising #308's run-scoped `UNIQUE(protocol_line_id,
/// run_id, day_number)` constraint (replacing the old protocol-line-scoped
/// one that would have collided across runs).
#[tokio::test]
async fn test_adherence_second_run_of_same_protocol_is_isolated() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_two_line_recipe(&app, &token, 10).await;
    let protocol_id = protocol["id"].as_str().unwrap().to_string();
    let line_a = protocol["lines"][0]["id"].as_str().unwrap().to_string();

    // Run 1: backdated 5 days, log day0 completed.
    let run1 = start_run(&app, &token, &protocol_id, 5).await;
    let run1_id = run1["id"].as_str().unwrap().to_string();
    log_dose(&app, &token, &run1_id, &line_a, 0).await;

    // Pause run1 so a second run can be active (list_active_runs concerns
    // are covered elsewhere; here we only need two runs to exist).
    app.app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/protocols/runs/{run1_id}"),
            &token,
            Some(&json!({"status": "paused"})),
        ))
        .await
        .unwrap();

    // Run 2 of the SAME protocol: backdated 2 days, log day0 SKIPPED this time.
    let run2 = start_run(&app, &token, &protocol_id, 2).await;
    let run2_id = run2["id"].as_str().unwrap().to_string();
    skip_dose(&app, &token, &run2_id, &line_a, 0).await;

    let adherence1 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/runs/{run1_id}/adherence"),
            &token,
            None,
        ))
        .await
        .unwrap();
    let json1 = common::body_json(adherence1).await;
    let line1 = find_line(json1["lines"].as_array().unwrap(), &line_a);
    assert_eq!(line1["completed"], 1);
    assert_eq!(line1["skipped"], 0);

    let adherence2 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/runs/{run2_id}/adherence"),
            &token,
            None,
        ))
        .await
        .unwrap();
    let json2 = common::body_json(adherence2).await;
    let line2 = find_line(json2["lines"].as_array().unwrap(), &line_a);
    assert_eq!(line2["completed"], 0);
    assert_eq!(line2["skipped"], 1);
}

// ---------------------------------------------------------------------------
// GET /protocols/runs/missed-doses
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_missed_doses_across_two_runs() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    // Run A: 3 days ago, daily line, nothing logged -> days 0,1,2 missed.
    let protocol_a = create_two_line_recipe(&app, &token, 10).await;
    let protocol_a_id = protocol_a["id"].as_str().unwrap().to_string();
    let run_a = start_run(&app, &token, &protocol_a_id, 3).await;
    let run_a_id = run_a["id"].as_str().unwrap().to_string();

    // Run B: a different protocol, 2 days ago, nothing logged -> days 0,1 missed.
    let protocol_b = create_two_line_recipe(&app, &token, 10).await;
    let protocol_b_id = protocol_b["id"].as_str().unwrap().to_string();
    let run_b = start_run(&app, &token, &protocol_b_id, 2).await;
    let run_b_id = run_b["id"].as_str().unwrap().to_string();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols/runs/missed-doses",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let items = common::body_json(resp).await;
    let items = items.as_array().unwrap();

    assert!(
        items.iter().any(|i| i["run_id"] == run_a_id),
        "expected missed doses from run A: {items:#?}"
    );
    assert!(
        items.iter().any(|i| i["run_id"] == run_b_id),
        "expected missed doses from run B: {items:#?}"
    );
    for item in items {
        assert_eq!(item["status"], "missed");
    }
}

#[tokio::test]
async fn test_missed_doses_excludes_other_users_runs() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;
    let (_uid2, token2) = common::create_test_user(&app).await;

    let protocol = create_two_line_recipe(&app, &token, 10).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    start_run(&app, &token, protocol_id, 3).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols/runs/missed-doses",
            &token2,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let items = common::body_json(resp).await;
    assert!(items.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_missed_doses_unauthenticated_returns_401() {
    let app = common::setup().await;

    let req = axum::http::Request::builder()
        .method("GET")
        .uri("/api/v1/protocols/runs/missed-doses")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
}

/// The endpoint caps at 200 rows even when far more scheduled days are
/// overdue — a long-duration, far-backdated single-line protocol produces
/// hundreds of missed candidates on its own.
#[tokio::test]
async fn test_missed_doses_cap_respected() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let duration_days = 365;
    let pattern: Vec<bool> = vec![true; duration_days as usize];
    let body = json!({
        "name": "Long Haul Protocol",
        "duration_days": duration_days,
        "lines": [{
            "substance": "Vitamin D",
            "schedule_pattern": pattern,
            "sort_order": 0
        }]
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/protocols",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let protocol = common::body_json(resp).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    // Started 300 days ago: today_day=300, so days 0..299 (300 days) are all
    // scheduled+missed for this single line — comfortably over the 200 cap.
    start_run(&app, &token, protocol_id, 300).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols/runs/missed-doses",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let items = common::body_json(resp).await;
    let items = items.as_array().unwrap();
    assert_eq!(items.len(), 200, "expected the 200-row cap to apply");
}

// ---------------------------------------------------------------------------
// RunResponse.adherence_pct / doses_missed on list_active_runs
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_active_runs_includes_adherence_fields() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;
    let (_protocol_id, run_id, _line_a) = seed_mid_run(&app, &token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols/runs/active",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let runs = common::body_json(resp).await;
    let runs = runs.as_array().unwrap();
    let run = runs
        .iter()
        .find(|r| r["id"] == run_id)
        .expect("seeded run should be active");

    assert_eq!(run["doses_missed"], 3);
    // completed(3) / (scheduled_so_far(8) - skipped(2)) * 100 = 50.0
    let pct = run["adherence_pct"].as_f64().unwrap();
    assert!((pct - 50.0).abs() < 1e-9);
}

#[tokio::test]
async fn test_create_run_response_includes_adherence_fields() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_two_line_recipe(&app, &token, 5).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/runs"),
            &token,
            Some(&json!({})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let run = common::body_json(resp).await;

    // Fresh run started today: today_day=0, so there is no closed day yet
    // (closed_bound = today_day - 1 = -1) — scheduled_so_far is 0 and
    // adherence_pct is null, not 0%. This is the fix for the
    // fresh-run-shows-0%-adherence problem: a run that's had zero chances
    // to be adherent isn't the same as one with 0% adherence.
    assert_eq!(run["doses_missed"], 0);
    assert!(run["adherence_pct"].is_null());
}

#[tokio::test]
async fn test_create_run_with_backdated_start_date_computes_real_adherence() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let duration_days = 10;
    let protocol = create_one_line_recipe(&app, &token, duration_days).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    // 5 days ago -> today_day=5, closed days 0..4, nothing logged.
    let start_date = (Utc::now() - Duration::days(5)).date_naive();
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/runs"),
            &token,
            Some(&json!({"start_date": start_date.to_string()})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let run = common::body_json(resp).await;

    // scheduled_so_far=5, completed=0, skipped=0 -> pct=0.0 (not null: the
    // denominator (5) is > 0, it's just that nothing was completed).
    assert_eq!(run["doses_missed"], 5);
    let pct = run["adherence_pct"].as_f64().unwrap();
    assert!((pct - 0.0).abs() < 1e-9);
}

// ---------------------------------------------------------------------------
// Cross-implementation agreement: /adherence, /doses, and /runs/active all
// derive from the same canonical rule (dose_status::compute_dose_status +
// closed_bound + adherence_pct) via independent code paths — a pure-Rust
// per-day loop for /doses, and SQL aggregates for /adherence and
// /runs/active. These tests assert all three agree, using scenarios that
// specifically exercise the closed-day boundary the fix round introduced.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cross_implementation_agreement_fully_elapsed_run() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let duration_days = 5;
    let protocol = create_two_line_recipe(&app, &token, duration_days).await;
    let protocol_id = protocol["id"].as_str().unwrap().to_string();
    let line_a = protocol["lines"][0]["id"].as_str().unwrap().to_string();
    let line_b = protocol["lines"][1]["id"].as_str().unwrap().to_string();

    // Started well before the run's duration ended: every scheduled day is closed.
    let run = start_run(&app, &token, &protocol_id, 10).await;
    let run_id = run["id"].as_str().unwrap().to_string();
    let start_date = (Utc::now() - Duration::days(10)).date_naive();

    // Line A (daily, 5 scheduled days): completed, skipped, gap, completed, skipped.
    log_dose(&app, &token, &run_id, &line_a, 0).await;
    skip_dose(&app, &token, &run_id, &line_a, 1).await;
    // day2 left unlogged -> missed
    log_dose(&app, &token, &run_id, &line_a, 3).await;
    skip_dose(&app, &token, &run_id, &line_a, 4).await;

    // Line B (every other day: 0, 2, 4): completed, gap, skipped.
    log_dose(&app, &token, &run_id, &line_b, 0).await;
    // day2 left unlogged -> missed
    skip_dose(&app, &token, &run_id, &line_b, 4).await;

    let today_day = (Utc::now().date_naive() - start_date).num_days();
    let doses = fetch_doses(&app, &token, &run_id, 0, duration_days - 1).await;
    let adherence = fetch_adherence(&app, &token, &run_id).await;
    let active_runs = fetch_active_runs(&app, &token).await;
    let active_run = active_runs
        .iter()
        .find(|r| r["id"] == run_id)
        .expect("run should be active");

    // Expected (worked by hand): line A scheduled=5 completed=2 skipped=2
    // missed=1; line B scheduled=3 completed=1 skipped=1 missed=1. Totals:
    // scheduled=8 completed=3 skipped=3 missed=2;
    // pct = 3 / (8 - 3) * 100 = 60.0.
    assert_eq!(adherence["scheduled_so_far"], 8);
    assert_eq!(adherence["completed"], 3);
    assert_eq!(adherence["skipped"], 3);
    assert_eq!(adherence["missed"], 2);
    let pct = adherence["adherence_pct"].as_f64().unwrap();
    assert!((pct - 60.0).abs() < 1e-9);

    let (d_scheduled, d_completed, d_skipped, d_missed) =
        derive_adherence_from_doses(&doses, today_day);
    assert_eq!(d_scheduled, adherence["scheduled_so_far"]);
    assert_eq!(d_completed, adherence["completed"]);
    assert_eq!(d_skipped, adherence["skipped"]);
    assert_eq!(d_missed, adherence["missed"]);

    assert_eq!(active_run["doses_missed"], d_missed);
    let active_pct = active_run["adherence_pct"].as_f64().unwrap();
    assert!((active_pct - 60.0).abs() < 1e-9);
}

#[tokio::test]
async fn test_cross_implementation_agreement_tolerance_day_dose() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let duration_days = 5;
    let protocol = create_one_line_recipe(&app, &token, duration_days).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap().to_string();

    // 1 day ago -> today_day=1. day2 is the write path's "today_day + 1"
    // tolerance day.
    let run = start_run(&app, &token, protocol_id, 1).await;
    let run_id = run["id"].as_str().unwrap().to_string();
    let start_date = (Utc::now() - Duration::days(1)).date_naive();

    // day0 (the only closed day) is left unlogged -> missed.
    // day2 (tolerance day, not closed) is logged -> completed in /doses,
    // but must NOT count toward adherence.
    log_dose(&app, &token, &run_id, &line_id, 2).await;

    let today_day = (Utc::now().date_naive() - start_date).num_days();
    let doses = fetch_doses(&app, &token, &run_id, 0, duration_days - 1).await;
    let adherence = fetch_adherence(&app, &token, &run_id).await;

    // The tolerance-day dose is visible in /doses with its real status...
    let tolerance_entry = doses
        .iter()
        .find(|d| d["day_number"] == 2)
        .expect("tolerance-day entry should be present");
    assert_eq!(tolerance_entry["status"], "completed");
    assert!(tolerance_entry["dose_id"].as_str().is_some());

    // ...but /adherence only has one closed day (day0), which is missed —
    // the tolerance-day completion does not roll in yet.
    assert_eq!(adherence["scheduled_so_far"], 1);
    assert_eq!(adherence["completed"], 0);
    assert_eq!(adherence["skipped"], 0);
    assert_eq!(adherence["missed"], 1);
    let pct = adherence["adherence_pct"].as_f64().unwrap();
    assert!((pct - 0.0).abs() < 1e-9);

    let (d_scheduled, d_completed, d_skipped, d_missed) =
        derive_adherence_from_doses(&doses, today_day);
    assert_eq!(d_scheduled, adherence["scheduled_so_far"]);
    assert_eq!(d_completed, adherence["completed"]);
    assert_eq!(d_skipped, adherence["skipped"]);
    assert_eq!(d_missed, adherence["missed"]);
}

#[tokio::test]
async fn test_cross_implementation_agreement_messy_run() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let duration_days = 10;
    // Daily pattern except day2, which is deliberately NOT scheduled — a
    // mid-pattern false day must never appear anywhere, closed or not.
    let mut pattern = vec![true; duration_days as usize];
    pattern[2] = false;

    let body = json!({
        "name": "Messy Run Stack",
        "duration_days": duration_days,
        "lines": [{
            "substance": "Creatine",
            "dose": 5.0,
            "unit": "g",
            "schedule_pattern": pattern,
            "sort_order": 0
        }]
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/protocols",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let protocol = common::body_json(resp).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap().to_string();

    // 4 days ago -> today_day=4.
    let run = start_run(&app, &token, protocol_id, 4).await;
    let run_id = run["id"].as_str().unwrap().to_string();
    let start_date = (Utc::now() - Duration::days(4)).date_naive();

    log_dose(&app, &token, &run_id, &line_id, 0).await; // closed, completed
    skip_dose(&app, &token, &run_id, &line_id, 1).await; // closed, skipped
    // day2: not scheduled at all (pattern false) — no action, must never appear.
    // day3: closed, gap -> missed.
    log_dose(&app, &token, &run_id, &line_id, 4).await; // today-log: not closed
    log_dose(&app, &token, &run_id, &line_id, 5).await; // tolerance-day log: not closed
    // days 6-9: future gaps -> pending, not closed.

    let today_day = (Utc::now().date_naive() - start_date).num_days();
    let doses = fetch_doses(&app, &token, &run_id, 0, duration_days - 1).await;
    let adherence = fetch_adherence(&app, &token, &run_id).await;
    let active_runs = fetch_active_runs(&app, &token).await;
    let active_run = active_runs
        .iter()
        .find(|r| r["id"] == run_id)
        .expect("run should be active");

    // day2 must not appear in /doses under any circumstance.
    assert!(
        !doses.iter().any(|d| d["day_number"] == 2),
        "unscheduled day2 must never appear in /doses: {doses:#?}"
    );
    // today-log and tolerance-day log are visible with their real status.
    assert_eq!(
        doses.iter().find(|d| d["day_number"] == 4).unwrap()["status"],
        "completed"
    );
    assert_eq!(
        doses.iter().find(|d| d["day_number"] == 5).unwrap()["status"],
        "completed"
    );

    // Closed days (0,1,3 — day2 excluded): scheduled=3, completed=1 (day0),
    // skipped=1 (day1), missed=1 (day3). pct = 1 / (3 - 1) * 100 = 50.0.
    assert_eq!(adherence["scheduled_so_far"], 3);
    assert_eq!(adherence["completed"], 1);
    assert_eq!(adherence["skipped"], 1);
    assert_eq!(adherence["missed"], 1);
    let pct = adherence["adherence_pct"].as_f64().unwrap();
    assert!((pct - 50.0).abs() < 1e-9);

    let (d_scheduled, d_completed, d_skipped, d_missed) =
        derive_adherence_from_doses(&doses, today_day);
    assert_eq!(d_scheduled, adherence["scheduled_so_far"]);
    assert_eq!(d_completed, adherence["completed"]);
    assert_eq!(d_skipped, adherence["skipped"]);
    assert_eq!(d_missed, adherence["missed"]);

    assert_eq!(active_run["doses_missed"], 1);
    let active_pct = active_run["adherence_pct"].as_f64().unwrap();
    assert!((active_pct - 50.0).abs() < 1e-9);
}

// ---------------------------------------------------------------------------
// Pause semantics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_patch_run_paused_then_resumed_records_pause_interval() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_one_line_recipe(&app, &token, 10).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let run = start_run(&app, &token, protocol_id, 0).await;
    let run_id_str = run["id"].as_str().unwrap().to_string();
    let run_id: uuid::Uuid = run_id_str.parse().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/protocols/runs/{run_id_str}"),
            &token,
            Some(&json!({"status": "paused"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    let today = chrono::Utc::now().date_naive();
    let row: (chrono::NaiveDate, Option<chrono::NaiveDate>) =
        sqlx::query_as("SELECT paused_on, resumed_on FROM run_pauses WHERE run_id = $1")
            .bind(run_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(row.0, today);
    assert!(
        row.1.is_none(),
        "resumed_on should be null while still paused"
    );

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/protocols/runs/{run_id_str}"),
            &token,
            Some(&json!({"status": "active"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    let row: (chrono::NaiveDate, Option<chrono::NaiveDate>) =
        sqlx::query_as("SELECT paused_on, resumed_on FROM run_pauses WHERE run_id = $1")
            .bind(run_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(
        row.1,
        Some(today),
        "resumed_on should be set after resuming"
    );
}

#[tokio::test]
async fn test_paused_interval_excludes_days_from_adherence_and_doses() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let duration_days = 10;
    let protocol = create_one_line_recipe(&app, &token, duration_days).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap().to_string();

    // 7 days ago -> today_day=7, closed days 0..6.
    let run = start_run(&app, &token, protocol_id, 7).await;
    let run_id_str = run["id"].as_str().unwrap().to_string();
    let run_id: uuid::Uuid = run_id_str.parse().unwrap();
    let start_date = (Utc::now() - Duration::days(7)).date_naive();

    log_dose(&app, &token, &run_id_str, &line_id, 0).await;

    // Directly seed a [2, 5) pause interval (days 2, 3, 4) — this can't be
    // reproduced through the PATCH endpoint in a fast-running test, since
    // paused_on/resumed_on there are always CURRENT_DATE at the moment of
    // the call, never an arbitrary day in the past.
    let paused_on = start_date + Duration::days(2);
    let resumed_on = start_date + Duration::days(5);
    sqlx::query("INSERT INTO run_pauses (run_id, paused_on, resumed_on) VALUES ($1, $2, $3)")
        .bind(run_id)
        .bind(paused_on)
        .bind(resumed_on)
        .execute(&app.pool)
        .await
        .unwrap();

    // Closed days 0..6 minus paused days {2,3,4}: {0,1,5,6}. day0 completed;
    // day1,5,6 gaps -> missed. scheduled=4, completed=1, skipped=0, missed=3.
    let adherence = fetch_adherence(&app, &token, &run_id_str).await;
    assert_eq!(adherence["scheduled_so_far"], 4);
    assert_eq!(adherence["completed"], 1);
    assert_eq!(adherence["skipped"], 0);
    assert_eq!(adherence["missed"], 3);

    let doses = fetch_doses(&app, &token, &run_id_str, 0, duration_days - 1).await;
    for paused_day in [2, 3, 4] {
        assert!(
            !doses.iter().any(|d| d["day_number"] == paused_day),
            "paused day {paused_day} must not appear in /doses at all: {doses:#?}"
        );
    }
    assert_eq!(
        doses.iter().find(|d| d["day_number"] == 0).unwrap()["status"],
        "completed"
    );
    assert_eq!(
        doses.iter().find(|d| d["day_number"] == 1).unwrap()["status"],
        "missed"
    );

    let active_runs = fetch_active_runs(&app, &token).await;
    let active_run = active_runs
        .iter()
        .find(|r| r["id"] == run_id_str)
        .expect("run should still be active");
    assert_eq!(active_run["doses_missed"], 3);
}
