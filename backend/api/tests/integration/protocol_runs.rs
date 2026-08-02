// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Integration tests for the Protocol Runs feature.
//!
//! Tests the recipe-vs-execution split: protocols are reusable recipes,
//! runs are executions with start_date, status, and dose logging.

use serde_json::{Value, json};
use tower::ServiceExt;

use crate::common;

/// Helper: create a protocol recipe (no start_date) with 2 lines.
async fn create_recipe(app: &common::TestApp, token: &str) -> Value {
    let body = json!({
        "name": "BPC Stack",
        "description": "BPC-157 + TB-500 healing protocol",
        "duration_days": 7,
        "lines": [
            {
                "substance": "BPC-157",
                "dose": 250.0,
                "unit": "mcg",
                "route": "SubQ",
                "time_of_day": "morning",
                "schedule_pattern": [true, true, true, true, true, true, true],
                "sort_order": 0
            },
            {
                "substance": "TB-500",
                "dose": 2.0,
                "unit": "mg",
                "route": "SubQ",
                "time_of_day": "morning",
                "schedule_pattern": [true, false, true, false, true, false, true],
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

// ---------------------------------------------------------------------------
// Recipe creation tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_create_protocol_without_start_date() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let json = create_recipe(&app, &token).await;

    assert_eq!(json["name"], "BPC Stack");
    assert!(
        json["start_date"].is_null(),
        "recipe should have null start_date"
    );
    assert_eq!(json["status"], "draft");
    assert!(json["runs"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_create_protocol_with_start_date_still_works() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let today = chrono::Utc::now().date_naive();
    let body = json!({
        "name": "Legacy Protocol",
        "start_date": today.to_string(),
        "duration_days": 3,
        "lines": [{
            "substance": "Creatine",
            "dose": 5.0,
            "unit": "g",
            "schedule_pattern": [true, true, true],
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
    let json = common::body_json(resp).await;
    assert_eq!(json["start_date"], today.to_string());
}

// ---------------------------------------------------------------------------
// Run lifecycle tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_start_run_on_protocol() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    let today = chrono::Utc::now().date_naive();
    let run_body = json!({});

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/runs"),
            &token,
            Some(&run_body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let run = common::body_json(resp).await;
    assert_eq!(run["protocol_id"], protocol_id);
    assert_eq!(run["start_date"], today.to_string());
    assert_eq!(run["status"], "active");
    assert!(!run["id"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_start_run_with_custom_date() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    let run_body = json!({
        "start_date": "2026-05-01",
        "notify": true,
        "notify_time": "08:00"
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/runs"),
            &token,
            Some(&run_body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let run = common::body_json(resp).await;
    assert_eq!(run["start_date"], "2026-05-01");
    assert_eq!(run["notify"], true);
    assert_eq!(run["notify_time"], "08:00");
}

#[tokio::test]
async fn test_multiple_runs_on_same_protocol() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    // Start first run
    let resp1 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/runs"),
            &token,
            Some(&json!({"start_date": "2026-01-01"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp1.status(), 201);

    // Start second run
    let resp2 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/runs"),
            &token,
            Some(&json!({"start_date": "2026-03-01"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp2.status(), 201);

    // List runs should show both
    let list_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/{protocol_id}/runs"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(list_resp.status(), 200);

    let runs = common::body_json(list_resp).await;
    assert_eq!(runs.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_list_active_runs() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    // Start a run
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

    // List active runs
    let list_resp = app
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
    assert_eq!(list_resp.status(), 200);

    let runs = common::body_json(list_resp).await;
    let items = runs.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["protocol_name"], "BPC Stack");
    assert_eq!(items[0]["status"], "active");
}

#[tokio::test]
async fn test_update_run_status() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    let run_resp = app
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
    let run = common::body_json(run_resp).await;
    let run_id = run["id"].as_str().unwrap();

    // Pause the run
    let patch_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/protocols/runs/{run_id}"),
            &token,
            Some(&json!({"status": "paused"})),
        ))
        .await
        .unwrap();
    assert_eq!(patch_resp.status(), 204);

    // Active runs should now be empty
    let active_resp = app
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
    let active = common::body_json(active_resp).await;
    assert!(active.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_update_run_invalid_status_returns_400() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    let run_resp = app
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
    let run = common::body_json(run_resp).await;
    let run_id = run["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/protocols/runs/{run_id}"),
            &token,
            Some(&json!({"status": "invalid"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

// ---------------------------------------------------------------------------
// Dose logging on runs
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_log_dose_on_run() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap();

    // Start run with today's date
    let run_resp = app
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
    let run = common::body_json(run_resp).await;
    let run_id = run["id"].as_str().unwrap();

    // Log dose on run
    let dose_body = json!({
        "protocol_line_id": line_id,
        "day_number": 0
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/log"),
            &token,
            Some(&dose_body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let dose = common::body_json(resp).await;
    assert_eq!(dose["status"], "completed");
    assert!(dose["intervention_id"].as_str().is_some());
}

#[tokio::test]
async fn test_skip_dose_on_run() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap();

    let run_resp = app
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
    let run = common::body_json(run_resp).await;
    let run_id = run["id"].as_str().unwrap();

    let skip_body = json!({
        "protocol_line_id": line_id,
        "day_number": 0
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/skip"),
            &token,
            Some(&skip_body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_log_duplicate_dose_on_run_returns_409() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap();

    let run_resp = app
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
    let run = common::body_json(run_resp).await;
    let run_id = run["id"].as_str().unwrap();

    let dose_body = json!({
        "protocol_line_id": line_id,
        "day_number": 0
    });

    // First log succeeds
    let resp1 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/log"),
            &token,
            Some(&dose_body),
        ))
        .await
        .unwrap();
    assert_eq!(resp1.status(), 200);

    // Second log should conflict (unique constraint on protocol_line_id + day_number)
    let resp2 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/log"),
            &token,
            Some(&dose_body),
        ))
        .await
        .unwrap();
    assert_eq!(resp2.status(), 409);
}

// ---------------------------------------------------------------------------
// Today's doses via runs
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_todays_doses_via_run() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    // Start run with today's date
    app.app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/runs"),
            &token,
            Some(&json!({})),
        ))
        .await
        .unwrap();

    // Get today's doses
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols/runs/todays-doses",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let doses = common::body_json(resp).await;
    let items = doses.as_array().unwrap();
    assert!(
        !items.is_empty(),
        "should have doses for today from active run"
    );
    assert_eq!(items[0]["substance"], "BPC-157");
    assert!(items[0]["run_id"].as_str().is_some());
}

#[tokio::test]
async fn test_todays_doses_empty_without_run() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    // Create a recipe but do NOT start a run
    create_recipe(&app, &token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols/runs/todays-doses",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let doses = common::body_json(resp).await;
    assert!(
        doses.as_array().unwrap().is_empty(),
        "should have no doses without an active run"
    );
}

#[tokio::test]
async fn test_paused_run_excluded_from_todays_doses() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    // Start run
    let run_resp = app
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
    let run = common::body_json(run_resp).await;
    let run_id = run["id"].as_str().unwrap();

    // Pause run
    app.app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/protocols/runs/{run_id}"),
            &token,
            Some(&json!({"status": "paused"})),
        ))
        .await
        .unwrap();

    // Todays doses should be empty
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols/runs/todays-doses",
            &token,
            None,
        ))
        .await
        .unwrap();
    let doses = common::body_json(resp).await;
    assert!(doses.as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Active substances
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_active_substances() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    // Start run
    app.app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/runs"),
            &token,
            Some(&json!({})),
        ))
        .await
        .unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols/active-substances",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let substances = common::body_json(resp).await;
    let items = substances.as_array().unwrap();
    assert_eq!(items.len(), 2);

    let names: Vec<&str> = items
        .iter()
        .filter_map(|i| i["substance"].as_str())
        .collect();
    assert!(names.contains(&"BPC-157"));
    assert!(names.contains(&"TB-500"));
}

#[tokio::test]
async fn test_active_substances_empty_without_run() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    create_recipe(&app, &token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols/active-substances",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let substances = common::body_json(resp).await;
    assert!(substances.as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// IDOR protection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cannot_start_run_on_other_users_protocol() {
    let app = common::setup().await;
    let (_uid_a, token_a) = common::create_test_user(&app).await;
    let (_uid_b, token_b) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token_a).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    // User B tries to start a run on User A's protocol
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/runs"),
            &token_b,
            Some(&json!({})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_cannot_update_other_users_run() {
    let app = common::setup().await;
    let (_uid_a, token_a) = common::create_test_user(&app).await;
    let (_uid_b, token_b) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token_a).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    let run_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/runs"),
            &token_a,
            Some(&json!({})),
        ))
        .await
        .unwrap();
    let run = common::body_json(run_resp).await;
    let run_id = run["id"].as_str().unwrap();

    // User B tries to pause User A's run
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/protocols/runs/{run_id}"),
            &token_b,
            Some(&json!({"status": "paused"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_cannot_log_dose_on_other_users_run() {
    let app = common::setup().await;
    let (_uid_a, token_a) = common::create_test_user(&app).await;
    let (_uid_b, token_b) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token_a).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap();

    let run_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/runs"),
            &token_a,
            Some(&json!({})),
        ))
        .await
        .unwrap();
    let run = common::body_json(run_resp).await;
    let run_id = run["id"].as_str().unwrap();

    // User B tries to log a dose on User A's run
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/log"),
            &token_b,
            Some(&json!({"protocol_line_id": line_id, "day_number": 0})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// ---------------------------------------------------------------------------
// Auth tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_runs_endpoints_require_auth() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    // Try creating run without auth
    let req = axum::body::Body::empty();
    let resp = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("POST")
                .uri(format!("/api/v1/protocols/{protocol_id}/runs"))
                .header("content-type", "application/json")
                .body(axum::body::Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);

    // Try listing active runs without auth
    let resp = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/api/v1/protocols/runs/active")
                .body(req)
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

// ---------------------------------------------------------------------------
// Protocol GET includes runs
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_protocol_includes_runs() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    // Start a run
    app.app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/runs"),
            &token,
            Some(&json!({})),
        ))
        .await
        .unwrap();

    // Get protocol should include runs
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/{protocol_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let json = common::body_json(resp).await;
    let runs = json["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["status"], "active");
}

// ---------------------------------------------------------------------------
// Dose tracking foundation: per-run scoping, timestamps, validation, undo
// ---------------------------------------------------------------------------

/// Start a run on `protocol_id` (defaults to today) and return the parsed
/// run JSON.
async fn start_run(app: &common::TestApp, token: &str, protocol_id: &str) -> Value {
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/runs"),
            token,
            Some(&json!({})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    common::body_json(resp).await
}

#[tokio::test]
async fn test_second_run_of_same_protocol_logs_same_day_without_collision() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap();

    let run1 = start_run(&app, &token, protocol_id).await;
    let run1_id = run1["id"].as_str().unwrap();

    let dose_body = json!({"protocol_line_id": line_id, "day_number": 0});

    let resp1 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run1_id}/doses/log"),
            &token,
            Some(&dose_body),
        ))
        .await
        .unwrap();
    assert_eq!(resp1.status(), 200);

    // Start a second run of the SAME protocol and log the same day_number —
    // this must not collide with the first run's dose (the old
    // UNIQUE(protocol_line_id, day_number) constraint would 409 here).
    let run2 = start_run(&app, &token, protocol_id).await;
    let run2_id = run2["id"].as_str().unwrap();
    assert_ne!(run1_id, run2_id);

    let resp2 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run2_id}/doses/log"),
            &token,
            Some(&dose_body),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp2.status(),
        200,
        "second run should log day 0 independently of the first run"
    );
}

#[tokio::test]
async fn test_legacy_null_run_dose_unaffected_by_run_scoped_dose() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    // A legacy protocol with its own start_date (pre-runs style).
    let today = chrono::Utc::now().date_naive();
    let body = json!({
        "name": "Legacy Protocol",
        "start_date": today.to_string(),
        "duration_days": 3,
        "lines": [{
            "substance": "Creatine",
            "dose": 5.0,
            "unit": "g",
            "schedule_pattern": [true, true, true],
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
    let protocol = common::body_json(resp).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap();

    // Log via the legacy protocol-level endpoint (run_id stays NULL).
    let dose_body = json!({"protocol_line_id": line_id, "day_number": 0});
    let legacy_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/doses/log"),
            &token,
            Some(&dose_body),
        ))
        .await
        .unwrap();
    assert_eq!(legacy_resp.status(), 200);
    let legacy_dose = common::body_json(legacy_resp).await;
    assert!(legacy_dose["run_id"].is_null());

    // A duplicate legacy log for the same day still conflicts (NULL-run rows
    // remain unique among themselves via NULLS NOT DISTINCT).
    let legacy_dup = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{protocol_id}/doses/log"),
            &token,
            Some(&dose_body),
        ))
        .await
        .unwrap();
    assert_eq!(legacy_dup.status(), 409);

    // Now start a real run on the same protocol and log the same
    // line/day_number — it must succeed because run_id differs (Some vs None).
    let run = start_run(&app, &token, protocol_id).await;
    let run_id = run["id"].as_str().unwrap();

    let run_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/log"),
            &token,
            Some(&dose_body),
        ))
        .await
        .unwrap();
    assert_eq!(
        run_resp.status(),
        200,
        "a run-scoped dose must not collide with the legacy NULL-run dose"
    );
}

#[tokio::test]
async fn test_default_administered_at_derived_from_time_of_day() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    // BPC-157 line has time_of_day "morning" (not AM/PM) -> defaults to noon.
    // TB-500 line also has time_of_day "morning".
    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap();

    let run = start_run(&app, &token, protocol_id).await;
    let run_id = run["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/log"),
            &token,
            Some(&json!({"protocol_line_id": line_id, "day_number": 0})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let dose = common::body_json(resp).await;
    let intervention_id = dose["intervention_id"].as_str().unwrap();

    let get_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/interventions/{intervention_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    let intervention = common::body_json(get_resp).await;
    let administered_at = intervention["administered_at"].as_str().unwrap();
    // time_of_day is not "AM"/"PM" -> default noon.
    assert!(
        administered_at.ends_with("T12:00:00Z"),
        "expected noon default, got {administered_at}"
    );
}

#[tokio::test]
async fn test_am_pm_default_administered_at_timestamps() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let body = json!({
        "name": "AM/PM Stack",
        "duration_days": 2,
        "lines": [
            {
                "substance": "Morning Vitamin",
                "schedule_pattern": [true, true],
                "time_of_day": "AM",
                "sort_order": 0
            },
            {
                "substance": "Evening Magnesium",
                "schedule_pattern": [true, true],
                "time_of_day": "PM",
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
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    let protocol = common::body_json(resp).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let am_line_id = protocol["lines"][0]["id"].as_str().unwrap();
    let pm_line_id = protocol["lines"][1]["id"].as_str().unwrap();

    let run = start_run(&app, &token, protocol_id).await;
    let run_id = run["id"].as_str().unwrap();

    for (line_id, expected_suffix) in [(am_line_id, "T08:00:00Z"), (pm_line_id, "T20:00:00Z")] {
        let resp = app
            .app
            .clone()
            .oneshot(common::auth_request(
                "POST",
                &format!("/api/v1/protocols/runs/{run_id}/doses/log"),
                &token,
                Some(&json!({"protocol_line_id": line_id, "day_number": 0})),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let dose = common::body_json(resp).await;
        let intervention_id = dose["intervention_id"].as_str().unwrap();

        let get_resp = app
            .app
            .clone()
            .oneshot(common::auth_request(
                "GET",
                &format!("/api/v1/interventions/{intervention_id}"),
                &token,
                None,
            ))
            .await
            .unwrap();
        let intervention = common::body_json(get_resp).await;
        let administered_at = intervention["administered_at"].as_str().unwrap();
        assert!(
            administered_at.ends_with(expected_suffix),
            "expected {expected_suffix}, got {administered_at}"
        );
    }
}

#[tokio::test]
async fn test_log_dose_with_explicit_administered_at_on_date() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap();

    let run = start_run(&app, &token, protocol_id).await;
    let run_id = run["id"].as_str().unwrap();
    let start_date = run["start_date"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/log"),
            &token,
            Some(&json!({
                "protocol_line_id": line_id,
                "day_number": 0,
                "administered_at": format!("{start_date}T09:15:00Z"),
                "notes": "logged a bit late"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let dose = common::body_json(resp).await;
    let intervention_id = dose["intervention_id"].as_str().unwrap();

    let get_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/interventions/{intervention_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    let intervention = common::body_json(get_resp).await;
    assert_eq!(
        intervention["administered_at"],
        format!("{start_date}T09:15:00Z")
    );
    assert_eq!(intervention["notes"], "logged a bit late");
}

#[tokio::test]
async fn test_log_dose_administered_at_off_date_returns_400() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap();

    let run = start_run(&app, &token, protocol_id).await;
    let run_id = run["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/log"),
            &token,
            Some(&json!({
                "protocol_line_id": line_id,
                "day_number": 0,
                "administered_at": "2099-01-01T09:00:00Z"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_log_dose_for_future_day_returns_400() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap();

    let run = start_run(&app, &token, protocol_id).await;
    let run_id = run["id"].as_str().unwrap();

    // day_number 1 is tomorrow relative to a run started today.
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/log"),
            &token,
            Some(&json!({"protocol_line_id": line_id, "day_number": 1})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_skip_dose_with_reason_round_trips() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap();

    let run = start_run(&app, &token, protocol_id).await;
    let run_id = run["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/skip"),
            &token,
            Some(&json!({
                "protocol_line_id": line_id,
                "day_number": 0,
                "skip_reason": "traveling, forgot supplies"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Confirm the skip_reason round-trips through the protocol detail view.
    let get_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/{protocol_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    let protocol = common::body_json(get_resp).await;
    let doses = protocol["lines"][0]["doses"].as_array().unwrap();
    let skipped = doses
        .iter()
        .find(|d| d["day_number"] == 0)
        .expect("dose for day 0 should exist");
    assert_eq!(skipped["status"], "skipped");
    assert_eq!(skipped["skip_reason"], "traveling, forgot supplies");
}

#[tokio::test]
async fn test_delete_dose_removes_dose_and_intervention() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap();

    let run = start_run(&app, &token, protocol_id).await;
    let run_id = run["id"].as_str().unwrap();

    let log_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/log"),
            &token,
            Some(&json!({"protocol_line_id": line_id, "day_number": 0})),
        ))
        .await
        .unwrap();
    let dose = common::body_json(log_resp).await;
    let dose_id = dose["id"].as_str().unwrap();
    let intervention_id = dose["intervention_id"].as_str().unwrap();

    let delete_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/protocols/runs/{run_id}/doses/{dose_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), 204);

    // The intervention it created is gone too.
    let get_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/interventions/{intervention_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 404);

    // Re-logging the same day now succeeds (no leftover unique-constraint row).
    let relog_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/log"),
            &token,
            Some(&json!({"protocol_line_id": line_id, "day_number": 0})),
        ))
        .await
        .unwrap();
    assert_eq!(relog_resp.status(), 200);
}

#[tokio::test]
async fn test_delete_dose_not_found_for_other_user() {
    let app = common::setup().await;
    let (_uid_a, token_a) = common::create_test_user(&app).await;
    let (_uid_b, token_b) = common::create_test_user(&app).await;

    let protocol = create_recipe(&app, &token_a).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap();

    let run = start_run(&app, &token_a, protocol_id).await;
    let run_id = run["id"].as_str().unwrap();

    let log_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/log"),
            &token_a,
            Some(&json!({"protocol_line_id": line_id, "day_number": 0})),
        ))
        .await
        .unwrap();
    let dose = common::body_json(log_resp).await;
    let dose_id = dose["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/protocols/runs/{run_id}/doses/{dose_id}"),
            &token_b,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// ---------------------------------------------------------------------------
// Notification preferences
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_notification_preferences_default() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols/notifications",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let json = common::body_json(resp).await;
    assert_eq!(json["default_notify"], false);
    assert_eq!(json["repeat_reminders"], false);
    assert_eq!(json["repeat_interval_minutes"], 30);
}

#[tokio::test]
async fn test_update_notification_preferences() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let body = json!({
        "default_notify": true,
        "default_notify_times": ["08:00", "20:00"],
        "repeat_reminders": true,
        "repeat_interval_minutes": 15
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            "/api/v1/protocols/notifications",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let json = common::body_json(resp).await;
    assert_eq!(json["default_notify"], true);
    assert_eq!(json["repeat_reminders"], true);
    assert_eq!(json["repeat_interval_minutes"], 15);

    let times = json["default_notify_times"].as_array().unwrap();
    assert_eq!(times.len(), 2);
}

// ---------------------------------------------------------------------------
// Push tokens
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_register_push_token() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let body = json!({
        "device_token": "abc123device",
        "platform": "ios"
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/notifications/push-token",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let json = common::body_json(resp).await;
    assert_eq!(json["device_token"], "abc123device");
    assert_eq!(json["platform"], "ios");
}

#[tokio::test]
async fn test_register_push_token_invalid_platform_returns_400() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let body = json!({
        "device_token": "abc123",
        "platform": "android"
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/notifications/push-token",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_register_push_token_empty_token_returns_400() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let body = json!({
        "device_token": "",
        "platform": "ios"
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/notifications/push-token",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_register_push_token_upsert() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let body = json!({
        "device_token": "same-device",
        "platform": "ios"
    });

    // Register once
    let resp1 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/notifications/push-token",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp1.status(), 201);

    // Register same device again - should upsert, not error
    let resp2 = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/notifications/push-token",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp2.status(), 201);
}

#[tokio::test]
async fn test_delete_push_token() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    // Register
    let body = json!({
        "device_token": "to-delete",
        "platform": "ios"
    });
    app.app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/notifications/push-token",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    // Delete
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            "/api/v1/notifications/push-token/to-delete",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Delete again should 404
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            "/api/v1/notifications/push-token/to-delete",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}
