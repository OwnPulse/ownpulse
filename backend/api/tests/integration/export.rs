// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;

use crate::common;

#[tokio::test]
async fn test_export_json() {
    let app = common::setup().await;
    let (user_id, token) = common::create_test_user(&app).await;

    // Create some data so the export is not empty
    let hr_body = json!({
        "source": "manual",
        "record_type": "heart_rate",
        "value": 65.0,
        "unit": "bpm",
        // date-ok
        "start_time": "2026-03-18T10:00:00Z"
    });
    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/health-records",
            &token,
            Some(&hr_body),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 201);

    // Also create an intervention — regression coverage for the
    // InterventionRow SELECT staying in sync with its columns (a stale
    // SELECT missing a new non-Option column 500s the whole export).
    let intervention_body = json!({
        "substance": "caffeine",
        "dose": 100.0,
        "unit": "mg",
        // date-ok
        "administered_at": "2026-03-18T09:00:00Z"
    });
    let intervention_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/interventions",
            &token,
            Some(&intervention_body),
        ))
        .await
        .unwrap();
    assert_eq!(intervention_resp.status(), 201);

    // A daily checkin.
    let checkin_body = json!({
        // date-ok
        "date": "2026-03-18",
        "energy": 7,
        "mood": 6,
        "focus": 8,
        "recovery": 5,
        "libido": 4,
        "notes": "felt good"
    });
    let checkin_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/checkins",
            &token,
            Some(&checkin_body),
        ))
        .await
        .unwrap();
    assert_eq!(checkin_resp.status(), 201);

    // A lab result.
    let lab_body = json!({
        // date-ok
        "panel_date": "2026-03-10",
        "lab_name": "Quest",
        "marker": "TSH",
        "value": 2.1,
        "unit": "mIU/L",
        "reference_low": 0.4,
        "reference_high": 4.0,
        "source": "manual"
    });
    let lab_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/labs",
            &token,
            Some(&lab_body),
        ))
        .await
        .unwrap();
    assert_eq!(lab_resp.status(), 201);

    // A plain observation (context_tag).
    let observation_body = json!({
        "type": "context_tag",
        "name": "travel",
        // date-ok
        "start_time": "2026-03-18T00:00:00Z",
        "source": "manual"
    });
    let observation_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observations",
            &token,
            Some(&observation_body),
        ))
        .await
        .unwrap();
    assert_eq!(observation_resp.status(), 201);

    // Sleep — stored as an `observations` row with type = "sleep" (no
    // separate table), so it should already appear in `observations`.
    let sleep_body = json!({
        // date-ok
        "date": "2026-03-17",
        "duration_minutes": 420,
        "deep_minutes": 90,
        "score": 82
    });
    let sleep_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/sleep",
            &token,
            Some(&sleep_body),
        ))
        .await
        .unwrap();
    assert_eq!(sleep_resp.status(), 201);

    // A protocol (with a line), a run, one logged dose, and one skipped
    // dose with a skip_reason — exercises protocols, protocol_lines,
    // protocol_runs, and protocol_doses end to end (post-0032 schema:
    // run_id + skip_reason on protocol_doses).
    let protocol_body = json!({
        "name": "BPC Stack",
        "description": "Healing protocol",
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
    let protocol_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/protocols",
            &token,
            Some(&protocol_body),
        ))
        .await
        .unwrap();
    assert_eq!(protocol_resp.status(), 201);
    let protocol = common::body_json(protocol_resp).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap();
    let other_line_id = protocol["lines"][1]["id"].as_str().unwrap();

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
    assert_eq!(run_resp.status(), 201);
    let run = common::body_json(run_resp).await;
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
    assert_eq!(log_resp.status(), 200);

    let skip_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/runs/{run_id}/doses/skip"),
            &token,
            Some(&json!({
                "protocol_line_id": other_line_id,
                "day_number": 0,
                "skip_reason": "forgot"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(skip_resp.status(), 204);

    // A calendar_days aggregate — no create endpoint exists (it's written
    // only by the Google Calendar sync job), so insert directly.
    // date-ok
    api::db::calendar_days::upsert(&app.pool, user_id, "2026-03-18".parse().unwrap(), 2, 45)
        .await
        .unwrap();

    // Export JSON
    let export_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/export/json",
            &token,
            None,
        ))
        .await
        .unwrap();

    let export_status = export_resp.status();
    let body_text = common::body_string(export_resp).await;
    assert_eq!(export_status, 200, "export failed: {body_text}");

    let json: serde_json::Value = serde_json::from_str(&body_text).unwrap();
    assert_eq!(json["schema_version"], "0.3.0");
    assert!(json["health_records"].is_array());
    assert!(json["interventions"].is_array());
    assert!(json["daily_checkins"].is_array());
    assert!(json["lab_results"].is_array());
    assert!(json["observations"].is_array());
    assert!(json["protocols"].is_array());
    assert!(json["protocol_lines"].is_array());
    assert!(json["protocol_runs"].is_array());
    assert!(json["protocol_doses"].is_array());
    assert!(json["calendar_days"].is_array());

    assert!(
        !json["health_records"].as_array().unwrap().is_empty(),
        "export should contain the health record we created"
    );
    assert!(
        !json["interventions"].as_array().unwrap().is_empty(),
        "export should contain the intervention we created"
    );
    assert_eq!(json["interventions"][0]["substance"], "caffeine");
    assert!(json["interventions"][0]["updated_at"].is_string());

    assert!(
        !json["daily_checkins"].as_array().unwrap().is_empty(),
        "export should contain the checkin we created"
    );
    assert_eq!(json["daily_checkins"][0]["mood"], 6);

    assert!(
        !json["lab_results"].as_array().unwrap().is_empty(),
        "export should contain the lab result we created"
    );
    assert_eq!(json["lab_results"][0]["marker"], "TSH");

    let observations = json["observations"].as_array().unwrap();
    assert!(
        observations
            .iter()
            .any(|o| o["type"] == "context_tag" && o["name"] == "travel"),
        "export should contain the plain observation we created"
    );
    assert!(
        observations.iter().any(|o| o["type"] == "sleep"),
        "sleep is stored as an observations row (type = 'sleep') and must \
         appear in the observations array — there is no separate sleep table"
    );

    let protocols = json["protocols"].as_array().unwrap();
    assert_eq!(
        protocols.len(),
        1,
        "export should contain the protocol we created"
    );
    assert_eq!(protocols[0]["id"], protocol_id);
    assert_eq!(protocols[0]["name"], "BPC Stack");

    let calendar_days = json["calendar_days"].as_array().unwrap();
    assert_eq!(
        calendar_days.len(),
        1,
        "export should contain the calendar_days row we inserted"
    );
    assert_eq!(calendar_days[0]["meeting_count"], 2);
    assert_eq!(calendar_days[0]["meeting_minutes"], 45);

    let lines = json["protocol_lines"].as_array().unwrap();
    assert_eq!(lines.len(), 2, "export should contain both protocol lines");
    assert!(lines.iter().any(|l| l["substance"] == "BPC-157"));
    assert!(lines.iter().any(|l| l["substance"] == "TB-500"));

    let runs = json["protocol_runs"].as_array().unwrap();
    assert_eq!(
        runs.len(),
        1,
        "export should contain the protocol run we created"
    );
    assert_eq!(runs[0]["id"], run_id);
    assert_eq!(runs[0]["protocol_id"], protocol_id);

    let doses = json["protocol_doses"].as_array().unwrap();
    assert_eq!(
        doses.len(),
        2,
        "export should contain both the logged and skipped dose"
    );
    let logged = doses
        .iter()
        .find(|d| d["status"] == "completed")
        .expect("logged dose should be present");
    assert_eq!(logged["run_id"], run_id);
    assert!(logged["intervention_id"].as_str().is_some());
    let skipped = doses
        .iter()
        .find(|d| d["status"] == "skipped")
        .expect("skipped dose should be present");
    assert_eq!(skipped["run_id"], run_id);
    assert_eq!(skipped["skip_reason"], "forgot");
}

/// Regression coverage: a user's export must never include another user's
/// protocol data — including via the two queries that reach `protocols`
/// through a JOIN (`protocol_runs`, `protocol_doses`) rather than a direct
/// `user_id` column, and including a legacy (pre-runs) `run_id = NULL` dose.
#[tokio::test]
async fn test_export_json_excludes_other_users_protocols_and_doses() {
    let app = common::setup().await;
    let (_uid_a, token_a) = common::create_test_user(&app).await;
    let (_uid_b, token_b) = common::create_test_user(&app).await;

    // User B: a recipe + run-scoped logged dose.
    let protocol_body = json!({
        "name": "User B's Protocol",
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
            &token_b,
            Some(&protocol_body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let protocol = common::body_json(resp).await;
    let protocol_id = protocol["id"].as_str().unwrap();
    let line_id = protocol["lines"][0]["id"].as_str().unwrap();

    let run_resp = app
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
    assert_eq!(run_resp.status(), 201);
    let run = common::body_json(run_resp).await;
    let run_id = run["id"].as_str().unwrap();

    let log_resp = app
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
    assert_eq!(log_resp.status(), 200);

    // User B: a second, legacy (pre-runs) recipe with a `start_date` and no
    // run at all — logging a dose against it directly via the deprecated
    // `/protocols/:id/doses/log` produces a `protocol_doses` row with
    // `run_id = NULL` (see `db::protocols::log_dose`).
    let today = chrono::Utc::now().date_naive();
    let legacy_body = json!({
        "name": "User B's Legacy Protocol",
        "start_date": today.to_string(),
        "duration_days": 3,
        "lines": [{
            "substance": "Magnesium",
            "dose": 200.0,
            "unit": "mg",
            "schedule_pattern": [true, true, true],
            "sort_order": 0
        }]
    });
    let legacy_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/protocols",
            &token_b,
            Some(&legacy_body),
        ))
        .await
        .unwrap();
    assert_eq!(legacy_resp.status(), 201);
    let legacy_protocol = common::body_json(legacy_resp).await;
    let legacy_protocol_id = legacy_protocol["id"].as_str().unwrap();
    let legacy_line_id = legacy_protocol["lines"][0]["id"].as_str().unwrap();

    let legacy_log_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/{legacy_protocol_id}/doses/log"),
            &token_b,
            Some(&json!({"protocol_line_id": legacy_line_id, "day_number": 0})),
        ))
        .await
        .unwrap();
    assert_eq!(legacy_log_resp.status(), 200);
    let legacy_dose = common::body_json(legacy_log_resp).await;
    assert!(
        legacy_dose["run_id"].is_null(),
        "sanity check: the legacy no-run protocol must log a run_id = NULL dose"
    );

    // Sanity check: user B's own export actually contains all of the above,
    // so the isolation assertions below aren't trivially true because the
    // setup silently failed.
    let b_export_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/export/json",
            &token_b,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(b_export_resp.status(), 200);
    let b_json = common::body_json(b_export_resp).await;
    assert_eq!(b_json["protocols"].as_array().unwrap().len(), 2);
    assert_eq!(b_json["protocol_runs"].as_array().unwrap().len(), 1);
    assert_eq!(b_json["protocol_doses"].as_array().unwrap().len(), 2);

    // User A's export must contain none of it.
    let export_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/export/json",
            &token_a,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(export_resp.status(), 200);

    let json = common::body_json(export_resp).await;
    assert!(
        json["protocols"].as_array().unwrap().is_empty(),
        "user A's export must not contain user B's protocols"
    );
    assert!(json["protocol_lines"].as_array().unwrap().is_empty());
    assert!(
        json["protocol_runs"].as_array().unwrap().is_empty(),
        "user A's export must not contain user B's protocol_runs (joined via protocol_id, not a direct user_id column on the exported rows alone)"
    );
    assert!(
        json["protocol_doses"].as_array().unwrap().is_empty(),
        "user A's export must not contain user B's protocol_doses — including the \
         run_id = NULL legacy dose, which can only be scoped via the \
         protocol_doses -> protocol_lines -> protocols join"
    );
}

/// Regression coverage: promoting a protocol to a template (admin-only,
/// sets `protocols.user_id = NULL`) must remove it from its former owner's
/// export — templates are shared library content, not any one user's data.
#[tokio::test]
async fn test_export_json_excludes_templates() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    let protocol_body = json!({
        "name": "Soon-To-Be Template",
        "duration_days": 3,
        "lines": [{
            "substance": "Vitamin D",
            "dose": 5000.0,
            "unit": "iu",
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
            Some(&protocol_body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let protocol = common::body_json(resp).await;
    let protocol_id = protocol["id"].as_str().unwrap();

    // Before promotion, the protocol is the user's own and appears in export.
    let before_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/export/json",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(before_resp.status(), 200);
    let before = common::body_json(before_resp).await;
    assert_eq!(before["protocols"].as_array().unwrap().len(), 1);

    let promote_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/admin/protocols/{protocol_id}/promote"),
            &admin_token,
            Some(&json!({"tags": ["vitamins"]})),
        ))
        .await
        .unwrap();
    assert_eq!(promote_resp.status(), 204);

    let after_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/export/json",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(after_resp.status(), 200);
    let after = common::body_json(after_resp).await;
    assert!(
        after["protocols"].as_array().unwrap().is_empty(),
        "a protocol promoted to a template (user_id = NULL) must no longer \
         appear in its former owner's export"
    );
    assert!(after["protocol_lines"].as_array().unwrap().is_empty());
}

/// Asserts `GET /export/json`'s top-level data keys exactly match
/// `schema/open-schema.json`'s declared data keys, so the schema file stays
/// load-bearing documentation rather than silently drifting from reality.
#[tokio::test]
async fn test_export_json_keys_match_open_schema() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    // Upload genetic data so `genetic_records` — normally omitted entirely
    // when a user has none — is present, letting this test compare against
    // the full declared key set rather than a subset.
    let genetics_fixture = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/genetics/test_23andme.txt"
    ))
    .expect("genetics fixture should be readable");
    let upload_resp = app
        .app
        .clone()
        .oneshot(common::multipart_upload_request(
            "/api/v1/genetics/upload",
            &token,
            "test_23andme.txt",
            &genetics_fixture,
        ))
        .await
        .unwrap();
    assert_eq!(upload_resp.status(), 201);

    let export_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/export/json",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(export_resp.status(), 200);
    let json = common::body_json(export_resp).await;

    let export_keys: std::collections::BTreeSet<String> = json
        .as_object()
        .expect("export body should be a JSON object")
        .keys()
        .filter(|k| !["schema_version", "exported_at"].contains(&k.as_str()))
        .cloned()
        .collect();

    let schema_text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schema/open-schema.json"
    ))
    .expect("schema/open-schema.json should be readable");
    let schema: serde_json::Value =
        serde_json::from_str(&schema_text).expect("schema/open-schema.json should be valid JSON");
    let schema_keys: std::collections::BTreeSet<String> = schema
        .as_object()
        .expect("schema/open-schema.json should be a JSON object")
        .keys()
        .filter(|k| {
            !["schema_version", "schema_url", "description", "exported_at"].contains(&k.as_str())
        })
        .cloned()
        .collect();

    assert_eq!(
        export_keys, schema_keys,
        "GET /export/json's top-level data keys must exactly match schema/open-schema.json's \
         declared data keys — update whichever one drifted"
    );
}

/// Empty-data export must still be a valid, well-formed JSON document.
#[tokio::test]
async fn test_export_json_empty_data() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let export_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/export/json",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(export_resp.status(), 200);

    let json = common::body_json(export_resp).await;
    assert_eq!(json["schema_version"], "0.3.0");
    for key in [
        "health_records",
        "interventions",
        "daily_checkins",
        "lab_results",
        "observations",
        "protocols",
        "protocol_lines",
        "protocol_runs",
        "protocol_doses",
        "calendar_days",
    ] {
        assert!(
            json[key].is_array(),
            "{key} should still be a (possibly empty) array with no data"
        );
        assert!(json[key].as_array().unwrap().is_empty());
    }
    // genetic_records is omitted entirely (not an empty array) when a user
    // has never uploaded genetic data.
    assert!(json.get("genetic_records").is_none());
}

#[tokio::test]
async fn test_export_csv() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    // Create a health record so CSV has data
    let hr_body = json!({
        "source": "manual",
        "record_type": "spo2",
        "value": 98.0,
        "unit": "%",
        // date-ok
        "start_time": "2026-03-18T11:00:00Z"
    });
    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/health-records",
            &token,
            Some(&hr_body),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 201);

    // Export CSV
    let export_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/export/csv",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(export_resp.status(), 200);

    let csv_body = common::body_string(export_resp).await;
    assert!(
        csv_body.starts_with("id,source,record_type,value,unit,start_time,end_time"),
        "CSV should start with the expected header row, got: {}",
        csv_body.lines().next().unwrap_or("")
    );
    // Should have at least header + 1 data row
    assert!(
        csv_body.lines().count() >= 2,
        "CSV should have at least 2 lines"
    );
}
