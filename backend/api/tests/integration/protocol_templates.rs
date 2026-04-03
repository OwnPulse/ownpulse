// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Integration tests for protocol export/import, templates, and admin operations.

use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

use crate::common;

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Create a minimal protocol via the API and return (protocol_id, protocol_name).
async fn create_protocol(app: &common::TestApp, token: &str) -> (Uuid, String) {
    let name = format!("test-protocol-{}", Uuid::new_v4());
    let body = json!({
        "name": name,
        "start_date": "2026-01-01",
        "duration_days": 7,
        "lines": [{
            "substance": "Creatine",
            "dose": 5.0,
            "unit": "g",
            "route": "oral",
            "time_of_day": "morning",
            "schedule_pattern": [true, true, true, true, true, true, true],
            "sort_order": 0,
        }],
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
    let json = common::body_json(resp).await;
    let id = Uuid::parse_str(json["id"].as_str().unwrap()).unwrap();
    (id, name)
}

// ─── Export/Import Tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_export_protocol_returns_json_file() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;
    let (protocol_id, name) = create_protocol(&app, &token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/protocols/{protocol_id}/export"),
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let content_type = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        content_type.contains("application/json"),
        "expected application/json, got {content_type}"
    );

    let disposition = resp
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        disposition.contains("attachment"),
        "expected attachment disposition, got {disposition}"
    );

    let body = common::body_json(resp).await;
    assert_eq!(body["schema"], "ownpulse-protocol/v1");
    assert_eq!(body["name"], name);
    assert!(body["lines"].as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn test_import_protocol_from_json() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let import_body = json!({
        "schema": "ownpulse-protocol/v1",
        "name": "Imported Protocol",
        "description": "From a JSON file",
        "tags": ["nootropic"],
        "duration_days": 7,
        "lines": [{
            "substance": "Magnesium",
            "dose": 400.0,
            "unit": "mg",
            "route": "oral",
            "time_of_day": "evening",
            "pattern": [true, true, true, true, true, true, true],
        }],
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/protocols/import",
            &token,
            Some(&import_body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let body = common::body_json(resp).await;
    assert_eq!(body["name"], "Imported Protocol");
    assert_eq!(body["duration_days"], 7);
    assert_eq!(body["lines"].as_array().unwrap().len(), 1);
    assert_eq!(body["lines"][0]["substance"], "Magnesium");
}

#[tokio::test]
async fn test_import_protocol_invalid_schema_returns_400() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let import_body = json!({
        "schema": "wrong-schema/v99",
        "name": "Bad Protocol",
        "tags": [],
        "duration_days": 7,
        "lines": [{
            "substance": "X",
            "pattern": [true, true, true, true, true, true, true],
        }],
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/protocols/import",
            &token,
            Some(&import_body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_import_protocol_pattern_shorthand_expands() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let import_body = json!({
        "schema": "ownpulse-protocol/v1",
        "name": "Daily Shorthand",
        "tags": [],
        "duration_days": 7,
        "lines": [{
            "substance": "Vitamin D",
            "dose": 5000.0,
            "unit": "IU",
            "pattern": "daily",
        }],
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/protocols/import",
            &token,
            Some(&import_body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let body = common::body_json(resp).await;

    let schedule = body["lines"][0]["schedule_pattern"]
        .as_array()
        .expect("schedule_pattern should be an array");
    assert_eq!(schedule.len(), 7);
    assert!(
        schedule.iter().all(|v| v.as_bool() == Some(true)),
        "daily pattern should expand to all true: {schedule:?}"
    );
}

// ─── Template Tests ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_templates_empty() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols/templates",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = common::body_json(resp).await;
    let templates = body.as_array().unwrap();
    assert!(templates.is_empty());
}

#[tokio::test]
async fn test_list_templates_after_promote() {
    let app = common::setup().await;
    let (_user_id, user_token) = common::create_test_user(&app).await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (protocol_id, name) = create_protocol(&app, &user_token).await;

    // Admin promotes the protocol to a template
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/admin/protocols/{protocol_id}/promote"),
            &admin_token,
            Some(&json!({"tags": ["supplement"]})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // List templates should now include it
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols/templates",
            &user_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = common::body_json(resp).await;
    let templates = body.as_array().unwrap();
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0]["name"], name);
}

#[tokio::test]
async fn test_copy_template() {
    let app = common::setup().await;
    let (_user_id, user_token) = common::create_test_user(&app).await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (protocol_id, _name) = create_protocol(&app, &user_token).await;

    // Promote to template
    app.app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/admin/protocols/{protocol_id}/promote"),
            &admin_token,
            Some(&json!({})),
        ))
        .await
        .unwrap();

    // A different user copies the template
    let (_user2_id, user2_token) = common::create_test_user(&app).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/templates/{protocol_id}/copy"),
            &user2_token,
            Some(&json!({"start_date": "2026-03-01"})),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let body = common::body_json(resp).await;
    assert_eq!(body["start_date"], "2026-03-01");
    assert_eq!(body["is_template"], false);

    // Verify the new protocol appears in user2's list
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols",
            &user2_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let list = common::body_json(resp).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_copy_non_template_returns_404() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;
    let (protocol_id, _name) = create_protocol(&app, &token).await;

    // Try to copy a regular (non-template) protocol via the templates endpoint
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/protocols/templates/{protocol_id}/copy"),
            &token,
            Some(&json!({"start_date": "2026-03-01"})),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}

// ─── Admin Tests ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_admin_promote_protocol() {
    let app = common::setup().await;
    let (_user_id, user_token) = common::create_test_user(&app).await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (protocol_id, _name) = create_protocol(&app, &user_token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/admin/protocols/{protocol_id}/promote"),
            &admin_token,
            Some(&json!({"tags": ["sleep", "recovery"]})),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 204);

    // Verify the protocol is now a template (promote sets user_id=NULL so we
    // check via the templates list endpoint instead of GET /protocols/:id).
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols/templates",
            &user_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = common::body_json(resp).await;
    let templates = body.as_array().unwrap();
    assert!(
        templates
            .iter()
            .any(|t| t["id"].as_str() == Some(&protocol_id.to_string())),
        "promoted protocol should appear in templates list"
    );
}

#[tokio::test]
async fn test_admin_demote_template() {
    let app = common::setup().await;
    let (_user_id, user_token) = common::create_test_user(&app).await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (protocol_id, _name) = create_protocol(&app, &user_token).await;

    // Promote first
    app.app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/admin/protocols/{protocol_id}/promote"),
            &admin_token,
            Some(&json!({})),
        ))
        .await
        .unwrap();

    // Demote
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/admin/protocols/{protocol_id}/demote"),
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 204);

    // Verify is_template is false — demoted protocol has user_id=NULL still,
    // so check via the templates list (it should be empty now).
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols/templates",
            &user_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = common::body_json(resp).await;
    let templates = body.as_array().unwrap();
    assert!(
        templates.is_empty(),
        "demoted protocol should not appear in templates list"
    );
}

#[tokio::test]
async fn test_admin_bulk_import() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;
    let (_user_id, user_token) = common::create_test_user(&app).await;

    let bulk_body = json!({
        "protocols": [
            {
                "schema": "ownpulse-protocol/v1",
                "name": "Bulk Template A",
                "description": "First template",
                "tags": ["nootropic"],
                "duration_days": 14,
                "lines": [{
                    "substance": "Alpha-GPC",
                    "dose": 300.0,
                    "unit": "mg",
                    "pattern": "daily",
                }],
            },
            {
                "schema": "ownpulse-protocol/v1",
                "name": "Bulk Template B",
                "description": "Second template",
                "tags": ["sleep"],
                "duration_days": 7,
                "lines": [{
                    "substance": "Melatonin",
                    "dose": 0.3,
                    "unit": "mg",
                    "pattern": "daily",
                }],
            },
        ],
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/protocols/import",
            &admin_token,
            Some(&bulk_body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = common::body_json(resp).await;
    assert_eq!(body["imported"], 2);

    // Verify templates are visible to any user
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/protocols/templates",
            &user_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let templates = common::body_json(resp).await;
    assert_eq!(templates.as_array().unwrap().len(), 2);
}

// ─── Auth Tests ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_non_admin_cannot_promote() {
    let app = common::setup().await;
    let (_user_id, user_token) = common::create_test_user(&app).await;
    let (protocol_id, _name) = create_protocol(&app, &user_token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/admin/protocols/{protocol_id}/promote"),
            &user_token,
            Some(&json!({})),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_non_admin_cannot_bulk_import() {
    let app = common::setup().await;
    let (_user_id, user_token) = common::create_test_user(&app).await;

    let bulk_body = json!({
        "protocols": [{
            "schema": "ownpulse-protocol/v1",
            "name": "Sneaky Template",
            "tags": [],
            "duration_days": 7,
            "lines": [{
                "substance": "X",
                "pattern": "daily",
            }],
        }],
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/admin/protocols/import",
            &user_token,
            Some(&bulk_body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 403);
}
