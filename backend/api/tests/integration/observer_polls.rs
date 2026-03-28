// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

use crate::common;

/// Create a test user with a specific email and return (user_id, jwt).
async fn create_user_with_email(app: &common::TestApp, email: &str) -> (Uuid, String) {
    let hash = bcrypt::hash("testpassword", 4).unwrap();
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO users (username, password_hash, auth_provider, email)
         VALUES ($1, $2, 'local', $3) RETURNING id",
    )
    .bind(format!("user-{}", Uuid::new_v4()))
    .bind(&hash)
    .bind(email)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject)
         VALUES ($1, 'local', $2)",
    )
    .bind(row.0)
    .bind(row.0.to_string())
    .execute(&app.pool)
    .await
    .unwrap();

    let token = api::auth::jwt::encode_access_token(
        row.0,
        "user",
        "test-jwt-secret-at-least-32-bytes-long",
        "http://localhost:5173",
        3600,
    )
    .unwrap();

    (row.0, token)
}

fn unique_email(prefix: &str) -> String {
    format!("{prefix}-{}@example.com", Uuid::new_v4())
}

// =============================================================================
// Poll lifecycle
// =============================================================================

#[tokio::test]
async fn create_poll_returns_201() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let body = json!({
        "name": "Daily wellness",
        "custom_prompt": "Rate me on these dimensions",
        "dimensions": ["energy", "mood", "focus"]
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let json = common::body_json(resp).await;
    assert_eq!(json["name"], "Daily wellness");
    assert_eq!(json["custom_prompt"], "Rate me on these dimensions");
    assert_eq!(json["dimensions"].as_array().unwrap().len(), 3);
    assert!(json["members"].as_array().unwrap().is_empty());
    assert!(!json["id"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn create_poll_strips_html_from_prompt() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let body = json!({
        "name": "Test",
        "custom_prompt": "<script>alert(1)</script>Please rate",
        "dimensions": ["energy"]
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let json = common::body_json(resp).await;
    assert_eq!(json["custom_prompt"], "alert(1)Please rate");
}

#[tokio::test]
async fn create_poll_rejects_empty_name() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let body = json!({
        "name": "",
        "dimensions": ["energy"]
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn create_poll_rejects_empty_dimensions() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let body = json!({
        "name": "Test",
        "dimensions": []
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn create_poll_rejects_invalid_dimension_chars() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let body = json!({
        "name": "Test",
        "dimensions": ["has-dash"]
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn create_poll_rejects_too_many_dimensions() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let dims: Vec<String> = (0..11).map(|i| format!("dim_{i}")).collect();
    let body = json!({
        "name": "Test",
        "dimensions": dims
    });

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn create_poll_unauthenticated_returns_401() {
    let app = common::setup().await;

    let body = json!({
        "name": "Test",
        "dimensions": ["energy"]
    });

    let req = http::Request::builder()
        .method("POST")
        .uri("/api/v1/observer-polls")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let resp = app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn list_polls_returns_owned_polls() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    // Create two polls
    for name in &["Poll A", "Poll B"] {
        let body = json!({
            "name": name,
            "dimensions": ["energy"]
        });
        let resp = app
            .app
            .clone()
            .oneshot(common::auth_request(
                "POST",
                "/api/v1/observer-polls",
                &token,
                Some(&body),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), 201);
    }

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/observer-polls",
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn get_poll_includes_members() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let body = json!({
        "name": "My poll",
        "dimensions": ["energy", "mood"]
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    let poll = common::body_json(resp).await;
    let poll_id = poll["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/observer-polls/{poll_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["name"], "My poll");
    assert!(json["members"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn update_poll_changes_name() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let body = json!({
        "name": "Original",
        "dimensions": ["energy"]
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    let poll = common::body_json(resp).await;
    let poll_id = poll["id"].as_str().unwrap();

    let update_body = json!({"name": "Updated"});
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/observer-polls/{poll_id}"),
            &token,
            Some(&update_body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["name"], "Updated");
}

#[tokio::test]
async fn soft_delete_poll_returns_204() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let body = json!({
        "name": "To delete",
        "dimensions": ["energy"]
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    let poll = common::body_json(resp).await;
    let poll_id = poll["id"].as_str().unwrap();

    // Delete
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/observer-polls/{poll_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // List should be empty
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/observer-polls",
            &token,
            None,
        ))
        .await
        .unwrap();
    let json = common::body_json(resp).await;
    assert!(json.as_array().unwrap().is_empty());

    // Get should return 404
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/observer-polls/{poll_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// =============================================================================
// Invite / accept
// =============================================================================

#[tokio::test]
async fn create_invite_returns_token_and_url() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let body = json!({
        "name": "Poll",
        "dimensions": ["energy"]
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();
    let poll = common::body_json(resp).await;
    let poll_id = poll["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/observer-polls/{poll_id}/invite"),
            &token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let json = common::body_json(resp).await;
    assert!(!json["invite_token"].as_str().unwrap().is_empty());
    assert!(json["invite_url"]
        .as_str()
        .unwrap()
        .contains("observer-polls/accept"));
    assert!(!json["invite_expires_at"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn accept_invite_valid_token_returns_accepted() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    // Create poll and invite
    let body = json!({"name": "Poll", "dimensions": ["energy"]});
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &owner_token,
            Some(&body),
        ))
        .await
        .unwrap();
    let poll = common::body_json(resp).await;
    let poll_id = poll["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/observer-polls/{poll_id}/invite"),
            &owner_token,
            None,
        ))
        .await
        .unwrap();
    let invite = common::body_json(resp).await;
    let invite_token = invite["invite_token"].as_str().unwrap();

    // Observer accepts
    let body = json!({"token": invite_token});
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls/accept",
            &observer_token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["status"], "accepted");
}

#[tokio::test]
async fn accept_invite_same_token_again_returns_acknowledged() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    let body = json!({"name": "Poll", "dimensions": ["energy"]});
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &owner_token,
            Some(&body),
        ))
        .await
        .unwrap();
    let poll = common::body_json(resp).await;
    let poll_id = poll["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/observer-polls/{poll_id}/invite"),
            &owner_token,
            None,
        ))
        .await
        .unwrap();
    let invite = common::body_json(resp).await;
    let invite_token = invite["invite_token"].as_str().unwrap();

    // First accept
    let body = json!({"token": invite_token});
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls/accept",
            &observer_token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(common::body_json(resp).await["status"], "accepted");

    // Second accept — should return acknowledged
    let body = json!({"token": invite_token});
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls/accept",
            &observer_token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["status"], "acknowledged");
}

#[tokio::test]
async fn accept_invite_random_uuid_returns_acknowledged() {
    let app = common::setup().await;
    let (_uid, token) = common::create_test_user(&app).await;

    let body = json!({"token": Uuid::new_v4().to_string()});
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls/accept",
            &token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["status"], "acknowledged");
}

#[tokio::test]
async fn accept_invite_expired_returns_acknowledged() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    let body = json!({"name": "Poll", "dimensions": ["energy"]});
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &owner_token,
            Some(&body),
        ))
        .await
        .unwrap();
    let poll = common::body_json(resp).await;
    let poll_id = poll["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/observer-polls/{poll_id}/invite"),
            &owner_token,
            None,
        ))
        .await
        .unwrap();
    let invite = common::body_json(resp).await;
    let invite_token = invite["invite_token"].as_str().unwrap();

    // Expire the invite in the DB
    sqlx::query("UPDATE observer_poll_members SET invite_expires_at = now() - INTERVAL '1 day' WHERE invite_token = $1::UUID")
        .bind(invite_token)
        .execute(&app.pool)
        .await
        .unwrap();

    let body = json!({"token": invite_token});
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls/accept",
            &observer_token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["status"], "acknowledged");
}

// =============================================================================
// Response submission
// =============================================================================

/// Helper: create poll, invite, and accept. Returns (poll_id, member_id).
async fn setup_poll_with_observer(
    app: &common::TestApp,
    owner_token: &str,
    observer_token: &str,
) -> String {
    let body = json!({
        "name": "Test poll",
        "dimensions": ["energy", "mood"]
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            owner_token,
            Some(&body),
        ))
        .await
        .unwrap();
    let poll = common::body_json(resp).await;
    let poll_id = poll["id"].as_str().unwrap().to_string();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/observer-polls/{poll_id}/invite"),
            owner_token,
            None,
        ))
        .await
        .unwrap();
    let invite = common::body_json(resp).await;
    let invite_token = invite["invite_token"].as_str().unwrap();

    let body = json!({"token": invite_token});
    app.app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls/accept",
            observer_token,
            Some(&body),
        ))
        .await
        .unwrap();

    poll_id
}

#[tokio::test]
async fn submit_response_valid_returns_201() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    let poll_id = setup_poll_with_observer(&app, &owner_token, &observer_token).await;

    let body = json!({
        "date": "2025-01-15",
        "scores": {"energy": 7, "mood": 8}
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/observer-polls/{poll_id}/respond"),
            &observer_token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let json = common::body_json(resp).await;
    assert_eq!(json["date"], "2025-01-15");
}

#[tokio::test]
async fn submit_response_same_date_returns_200_upsert() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    let poll_id = setup_poll_with_observer(&app, &owner_token, &observer_token).await;

    let body = json!({
        "date": "2025-01-15",
        "scores": {"energy": 7, "mood": 8}
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/observer-polls/{poll_id}/respond"),
            &observer_token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    // Submit again for the same date — should upsert
    let body = json!({
        "date": "2025-01-15",
        "scores": {"energy": 5, "mood": 6}
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/observer-polls/{poll_id}/respond"),
            &observer_token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json["scores"]["energy"], 5);
}

#[tokio::test]
async fn submit_response_invalid_dimension_returns_400() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    let poll_id = setup_poll_with_observer(&app, &owner_token, &observer_token).await;

    let body = json!({
        "date": "2025-01-15",
        "scores": {"energy": 7, "unknown": 5}
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/observer-polls/{poll_id}/respond"),
            &observer_token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn submit_response_score_zero_returns_400() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    let poll_id = setup_poll_with_observer(&app, &owner_token, &observer_token).await;

    let body = json!({
        "date": "2025-01-15",
        "scores": {"energy": 0}
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/observer-polls/{poll_id}/respond"),
            &observer_token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn submit_response_score_eleven_returns_400() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    let poll_id = setup_poll_with_observer(&app, &owner_token, &observer_token).await;

    let body = json!({
        "date": "2025-01-15",
        "scores": {"energy": 11}
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/observer-polls/{poll_id}/respond"),
            &observer_token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn submit_response_future_date_returns_400() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    let poll_id = setup_poll_with_observer(&app, &owner_token, &observer_token).await;

    let future_date = (chrono::Utc::now() + chrono::Duration::days(2))
        .format("%Y-%m-%d")
        .to_string();
    let body = json!({
        "date": future_date,
        "scores": {"energy": 5}
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/observer-polls/{poll_id}/respond"),
            &observer_token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn submit_response_non_member_returns_403() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_stranger_id, stranger_token) =
        create_user_with_email(&app, &unique_email("stranger")).await;

    let body = json!({"name": "Poll", "dimensions": ["energy"]});
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &owner_token,
            Some(&body),
        ))
        .await
        .unwrap();
    let poll = common::body_json(resp).await;
    let poll_id = poll["id"].as_str().unwrap();

    let body = json!({
        "date": "2025-01-15",
        "scores": {"energy": 5}
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/observer-polls/{poll_id}/respond"),
            &stranger_token,
            Some(&body),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 403);
}

// =============================================================================
// Owner views responses
// =============================================================================

#[tokio::test]
async fn owner_sees_responses_with_masked_email() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, "observer@example.com").await;

    let poll_id = setup_poll_with_observer(&app, &owner_token, &observer_token).await;

    // Submit a response
    let body = json!({
        "date": "2025-01-15",
        "scores": {"energy": 7, "mood": 8}
    });
    app.app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/observer-polls/{poll_id}/respond"),
            &observer_token,
            Some(&body),
        ))
        .await
        .unwrap();

    // Owner gets responses
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/observer-polls/{poll_id}/responses"),
            &owner_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let responses = json.as_array().unwrap();
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["scores"]["energy"], 7);
    // Email should be masked
    let email = responses[0]["observer_email"].as_str().unwrap();
    assert!(email.contains("***"));
    assert!(!email.contains("observer@"));
}

#[tokio::test]
async fn owner_filters_responses_by_date_range() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    let poll_id = setup_poll_with_observer(&app, &owner_token, &observer_token).await;

    // Submit responses for different dates
    for date in &["2025-01-10", "2025-01-15", "2025-01-20"] {
        let body = json!({
            "date": date,
            "scores": {"energy": 7, "mood": 8}
        });
        app.app
            .clone()
            .oneshot(common::auth_request(
                "PUT",
                &format!("/api/v1/observer-polls/{poll_id}/respond"),
                &observer_token,
                Some(&body),
            ))
            .await
            .unwrap();
    }

    // Filter by date range
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/observer-polls/{poll_id}/responses?start=2025-01-12&end=2025-01-18"),
            &owner_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let responses = json.as_array().unwrap();
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["date"], "2025-01-15");
}

// =============================================================================
// Observer data rights
// =============================================================================

#[tokio::test]
async fn observer_sees_own_responses() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    let poll_id = setup_poll_with_observer(&app, &owner_token, &observer_token).await;

    let body = json!({
        "date": "2025-01-15",
        "scores": {"energy": 7, "mood": 8}
    });
    app.app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/observer-polls/{poll_id}/respond"),
            &observer_token,
            Some(&body),
        ))
        .await
        .unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/observer-polls/{poll_id}/my-responses"),
            &observer_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    assert_eq!(json.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn observer_deletes_own_response() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    let poll_id = setup_poll_with_observer(&app, &owner_token, &observer_token).await;

    let body = json!({
        "date": "2025-01-15",
        "scores": {"energy": 7, "mood": 8}
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/observer-polls/{poll_id}/respond"),
            &observer_token,
            Some(&body),
        ))
        .await
        .unwrap();
    let response_json = common::body_json(resp).await;
    let response_id = response_json["id"].as_str().unwrap();

    // Delete the response
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/observer-polls/responses/{response_id}"),
            &observer_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify it's gone from my-responses
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/observer-polls/{poll_id}/my-responses"),
            &observer_token,
            None,
        ))
        .await
        .unwrap();
    let json = common::body_json(resp).await;
    assert!(json.as_array().unwrap().is_empty());

    // Verify it's gone from owner view too
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/observer-polls/{poll_id}/responses"),
            &owner_token,
            None,
        ))
        .await
        .unwrap();
    let json = common::body_json(resp).await;
    assert!(json.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn observer_exports_all_responses() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    let poll_id = setup_poll_with_observer(&app, &owner_token, &observer_token).await;

    let body = json!({
        "date": "2025-01-15",
        "scores": {"energy": 7, "mood": 8}
    });
    app.app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            &format!("/api/v1/observer-polls/{poll_id}/respond"),
            &observer_token,
            Some(&body),
        ))
        .await
        .unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/observer-polls/export",
            &observer_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let exports = json.as_array().unwrap();
    assert_eq!(exports.len(), 1);
    assert_eq!(exports[0]["poll_name"], "Test poll");
}

#[tokio::test]
async fn observer_my_polls_shows_accepted_polls() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    let _poll_id = setup_poll_with_observer(&app, &owner_token, &observer_token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/observer-polls/my-polls",
            &observer_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let polls = json.as_array().unwrap();
    assert_eq!(polls.len(), 1);
    assert_eq!(polls[0]["name"], "Test poll");
    // Owner email should be masked
    let owner_display = polls[0]["owner_display"].as_str().unwrap();
    assert!(owner_display.contains("***"));
}

// =============================================================================
// IDOR protection
// =============================================================================

#[tokio::test]
async fn idor_get_other_users_poll_returns_404() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_other_id, other_token) =
        create_user_with_email(&app, &unique_email("other")).await;

    let body = json!({"name": "Poll", "dimensions": ["energy"]});
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &owner_token,
            Some(&body),
        ))
        .await
        .unwrap();
    let poll = common::body_json(resp).await;
    let poll_id = poll["id"].as_str().unwrap();

    // Other user tries to GET
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/observer-polls/{poll_id}"),
            &other_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn idor_patch_other_users_poll_returns_404() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_other_id, other_token) =
        create_user_with_email(&app, &unique_email("other")).await;

    let body = json!({"name": "Poll", "dimensions": ["energy"]});
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &owner_token,
            Some(&body),
        ))
        .await
        .unwrap();
    let poll = common::body_json(resp).await;
    let poll_id = poll["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PATCH",
            &format!("/api/v1/observer-polls/{poll_id}"),
            &other_token,
            Some(&json!({"name": "Hacked"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn idor_delete_other_users_poll_returns_404() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_other_id, other_token) =
        create_user_with_email(&app, &unique_email("other")).await;

    let body = json!({"name": "Poll", "dimensions": ["energy"]});
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &owner_token,
            Some(&body),
        ))
        .await
        .unwrap();
    let poll = common::body_json(resp).await;
    let poll_id = poll["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/observer-polls/{poll_id}"),
            &other_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn idor_get_other_users_poll_responses_returns_404() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_other_id, other_token) =
        create_user_with_email(&app, &unique_email("other")).await;

    let body = json!({"name": "Poll", "dimensions": ["energy"]});
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &owner_token,
            Some(&body),
        ))
        .await
        .unwrap();
    let poll = common::body_json(resp).await;
    let poll_id = poll["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/observer-polls/{poll_id}/responses"),
            &other_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn idor_invite_to_other_users_poll_returns_404() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_other_id, other_token) =
        create_user_with_email(&app, &unique_email("other")).await;

    let body = json!({"name": "Poll", "dimensions": ["energy"]});
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/observer-polls",
            &owner_token,
            Some(&body),
        ))
        .await
        .unwrap();
    let poll = common::body_json(resp).await;
    let poll_id = poll["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/observer-polls/{poll_id}/invite"),
            &other_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// =============================================================================
// Cross-boundary checks
// =============================================================================

#[tokio::test]
async fn observer_cannot_access_owner_detail_endpoint() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    let poll_id = setup_poll_with_observer(&app, &owner_token, &observer_token).await;

    // Observer tries the owner GET detail endpoint
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/observer-polls/{poll_id}"),
            &observer_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn observer_cannot_access_owner_responses_endpoint() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    let poll_id = setup_poll_with_observer(&app, &owner_token, &observer_token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            &format!("/api/v1/observer-polls/{poll_id}/responses"),
            &observer_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn observer_my_polls_does_not_contain_health_data() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_user_with_email(&app, &unique_email("owner")).await;
    let (_observer_id, observer_token) =
        create_user_with_email(&app, &unique_email("observer")).await;

    let _poll_id = setup_poll_with_observer(&app, &owner_token, &observer_token).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/observer-polls/my-polls",
            &observer_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let json = common::body_json(resp).await;
    let polls = json.as_array().unwrap();
    assert_eq!(polls.len(), 1);

    // Verify the response shape only contains expected fields
    let poll = &polls[0];
    assert!(poll.get("id").is_some());
    assert!(poll.get("owner_display").is_some());
    assert!(poll.get("name").is_some());
    assert!(poll.get("dimensions").is_some());
    // Should NOT have owner health data fields
    assert!(poll.get("health_records").is_none());
    assert!(poll.get("checkins").is_none());
    assert!(poll.get("interventions").is_none());
    assert!(poll.get("members").is_none());
}
