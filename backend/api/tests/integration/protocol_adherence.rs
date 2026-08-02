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
    // Line A: scheduled_so_far=6 (days 0-5), completed=2, skipped=1, missed=2.
    // Line B: scheduled_so_far=3 (days 0,2,4), completed=1, skipped=1, missed=1.
    assert_eq!(json["scheduled_so_far"], 9);
    assert_eq!(json["completed"], 3);
    assert_eq!(json["skipped"], 2);
    assert_eq!(json["missed"], 3);
    let pct = json["adherence_pct"].as_f64().unwrap();
    assert!((pct - (3.0 / 9.0 * 100.0)).abs() < 1e-9);

    let lines = json["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 2);

    let a = find_line(lines, &line_a);
    assert_eq!(a["scheduled_so_far"], 6);
    assert_eq!(a["completed"], 2);
    assert_eq!(a["skipped"], 1);
    assert_eq!(a["missed"], 2);
    let a_pct = a["adherence_pct"].as_f64().unwrap();
    assert!((a_pct - (2.0 / 6.0 * 100.0)).abs() < 1e-9);

    let b = lines
        .iter()
        .find(|l| l["protocol_line_id"] != line_a)
        .unwrap();
    assert_eq!(b["scheduled_so_far"], 3);
    assert_eq!(b["completed"], 1);
    assert_eq!(b["skipped"], 1);
    assert_eq!(b["missed"], 1);
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
    let pct = run["adherence_pct"].as_f64().unwrap();
    assert!((pct - (3.0 / 9.0 * 100.0)).abs() < 1e-9);
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

    // Fresh run started today: nothing missed, and today is "pending" for
    // both lines so nothing completed either -> null pct (0 completed of
    // whatever is scheduled so far is still a defined percentage only when
    // scheduled_so_far > 0; a same-day run has scheduled_so_far >= 1 for
    // the daily line, so pct is Some(0.0) rather than null).
    assert_eq!(run["doses_missed"], 0);
    assert!(run["adherence_pct"].as_f64().unwrap() < 1e-9);
}
