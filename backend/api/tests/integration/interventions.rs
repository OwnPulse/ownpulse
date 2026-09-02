// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

use crate::common;

#[tokio::test]
async fn test_create_intervention() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "substance": "caffeine",
        "dose": 200.0,
        "unit": "mg",
        "route": "oral",
        // date-ok
        "administered_at": "2026-03-18T07:30:00Z"
    });

    let response = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/interventions",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 201);

    let json = common::body_json(response).await;
    assert_eq!(json["substance"], "caffeine");
    assert_eq!(json["dose"], 200.0);
    assert_eq!(json["unit"], "mg");
}

#[tokio::test]
async fn test_list_interventions() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "substance": "magnesium",
        "dose": 400.0,
        "unit": "mg",
        // date-ok
        "administered_at": "2026-03-18T21:00:00Z"
    });

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/interventions",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 201);

    let list_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/interventions",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(list_resp.status(), 200);

    let json = common::body_json(list_resp).await;
    let items = json.as_array().expect("response should be an array");
    assert!(!items.is_empty());
    assert!(items.iter().any(|i| i["substance"] == "magnesium"));
}

#[tokio::test]
async fn test_delete_intervention() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "substance": "vitamin_d",
        "dose": 5000.0,
        "unit": "IU",
        // date-ok
        "administered_at": "2026-03-18T08:00:00Z"
    });

    let create_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/interventions",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(create_resp.status(), 201);
    let created = common::body_json(create_resp).await;
    let id = created["id"].as_str().unwrap();

    // Delete
    let delete_resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/interventions/{id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(delete_resp.status(), 204);

    // Verify gone
    let get_resp = app
        .app
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/interventions/{id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(get_resp.status(), 404);
}

// ---------------------------------------------------------------------------
// PATCH /interventions/:id
// ---------------------------------------------------------------------------

async fn create_intervention(app: &common::TestApp, token: &str) -> serde_json::Value {
    let body = json!({
        "substance": "caffeine",
        "dose": 100.0,
        "unit": "mg",
        "route": "oral",
        // date-ok
        "administered_at": "2026-03-18T07:30:00Z"
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
    assert_eq!(resp.status(), 201);
    common::body_json(resp).await
}

#[tokio::test]
async fn test_update_intervention_happy_path() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let created = create_intervention(&app, &token).await;
    let id = created["id"].as_str().unwrap();
    let original_updated_at = created["updated_at"].as_str().unwrap().to_string();

    let patch_body = json!({
        "dose": 200.0,
        "notes": "doubled the dose"
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/interventions/{id}"),
            &token,
            Some(&patch_body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let updated = common::body_json(resp).await;
    assert_eq!(updated["dose"], 200.0);
    assert_eq!(updated["notes"], "doubled the dose");
    // Unset fields are left unchanged.
    assert_eq!(updated["substance"], "caffeine");
    assert_eq!(updated["unit"], "mg");
    assert_ne!(updated["updated_at"], original_updated_at);
}

#[tokio::test]
async fn test_update_intervention_requires_auth() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;
    let created = create_intervention(&app, &token).await;
    let id = created["id"].as_str().unwrap();

    let req = http::Request::builder()
        .method("PATCH")
        .uri(format!("/api/v1/interventions/{id}"))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(json!({"dose": 50.0}).to_string()))
        .unwrap();

    let resp = app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_update_intervention_cannot_touch_other_users() {
    let app = common::setup().await;
    let (_uid_a, token_a) = common::create_test_user(&app).await;
    let (_uid_b, token_b) = common::create_test_user(&app).await;

    let created = create_intervention(&app, &token_a).await;
    let id = created["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/interventions/{id}"),
            &token_b,
            Some(&json!({"dose": 999.0})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_update_intervention_empty_substance_returns_400() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;
    let created = create_intervention(&app, &token).await;
    let id = created["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/interventions/{id}"),
            &token,
            Some(&json!({"substance": "   "})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_update_intervention_not_found_returns_404() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/interventions/{}", Uuid::new_v4()),
            &token,
            Some(&json!({"dose": 1.0})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_synced_create_is_idempotent() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "substance": "levothyroxine",
        "unit": "count",
        // date-ok
        "administered_at": "2026-03-18T07:30:00Z",
        "source": "healthkit",
        "source_id": "dose-event-1"
    });

    let first = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/interventions",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(first.status(), 201);
    let first_json = common::body_json(first).await;
    assert_eq!(first_json["source"], "healthkit");
    assert_eq!(first_json["source_id"], "dose-event-1");

    let replay = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/interventions",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(replay.status(), 200);
    let replay_json = common::body_json(replay).await;
    assert_eq!(replay_json["id"], first_json["id"]);

    let list = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/interventions",
            &token,
            None,
        ))
        .await
        .unwrap();
    let list_json = common::body_json(list).await;
    assert_eq!(list_json.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_manual_creates_are_never_deduped() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "substance": "caffeine",
        "dose": 200.0,
        "unit": "mg",
        // date-ok
        "administered_at": "2026-03-18T07:30:00Z"
    });

    let mut ids = Vec::new();
    for _ in 0..2 {
        let resp = app
            .app
            .clone()
            .oneshot(common::auth_request(
                "POST",
                "/api/v1/interventions",
                &token,
                Some(&body),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), 201);
        let json = common::body_json(resp).await;
        assert_eq!(json["source"], "manual");
        assert_eq!(json["source_id"], serde_json::Value::Null);
        ids.push(json["id"].clone());
    }
    assert_ne!(ids[0], ids[1]);
}

#[tokio::test]
async fn test_source_id_is_scoped_per_user() {
    let app = common::setup().await;
    let (_user_a, token_a) = common::create_test_user(&app).await;
    let (_user_b, token_b) = common::create_test_user(&app).await;

    let body = json!({
        "substance": "levothyroxine",
        "unit": "count",
        // date-ok
        "administered_at": "2026-03-18T07:30:00Z",
        "source": "healthkit",
        "source_id": "dose-event-shared"
    });

    for token in [&token_a, &token_b] {
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
        assert_eq!(resp.status(), 201);
    }
}

#[tokio::test]
async fn test_export_includes_intervention_provenance() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "substance": "levothyroxine",
        "unit": "count",
        // date-ok
        "administered_at": "2026-03-18T07:30:00Z",
        "source": "healthkit",
        "source_id": "dose-event-export"
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/interventions",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let export = app
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
    assert_eq!(export.status(), 200);
    let json = common::body_json(export).await;
    let interventions = json["interventions"].as_array().unwrap();
    assert_eq!(interventions.len(), 1);
    assert_eq!(interventions[0]["source"], "healthkit");
    assert_eq!(interventions[0]["source_id"], "dose-event-export");
}

#[tokio::test]
async fn test_oversized_source_id_is_rejected() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "substance": "levothyroxine",
        "unit": "count",
        // date-ok
        "administered_at": "2026-03-18T07:30:00Z",
        "source": "healthkit",
        "source_id": "x".repeat(256)
    });

    let resp = app
        .app
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/interventions",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_replayed_create_does_not_publish_event() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let mut receiver = app.event_tx.subscribe();

    let body = json!({
        "substance": "levothyroxine",
        "unit": "count",
        // date-ok
        "administered_at": "2026-03-18T07:30:00Z",
        "source": "healthkit",
        "source_id": "dose-event-sse"
    });

    let first = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/interventions",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(first.status(), 201);
    receiver
        .try_recv()
        .expect("a fresh create must publish an event");

    let replay = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/interventions",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(replay.status(), 200);

    assert!(
        receiver.try_recv().is_err(),
        "a replayed create must not publish an event"
    );
}

#[tokio::test]
async fn test_source_defaults_to_manual_when_only_source_id_sent() {
    let app = common::setup().await;
    let (_user_id, token) = common::create_test_user(&app).await;

    let body = json!({
        "substance": "levothyroxine",
        "unit": "count",
        // date-ok
        "administered_at": "2026-03-18T07:30:00Z",
        "source_id": "dose-event-no-source"
    });

    let first = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/interventions",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(first.status(), 201);
    let first_json = common::body_json(first).await;
    assert_eq!(first_json["source"], "manual");

    let replay = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/interventions",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(replay.status(), 200);
    let replay_json = common::body_json(replay).await;
    assert_eq!(replay_json["id"], first_json["id"]);
}
