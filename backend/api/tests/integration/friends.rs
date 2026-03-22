// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

use crate::common;

fn unique_email(prefix: &str) -> String {
    format!("{prefix}-{}@test.com", Uuid::new_v4())
}

async fn create_test_user_with_email(app: &common::TestApp, email: &str) -> (Uuid, String) {
    let hash = bcrypt::hash("testpassword", 4).unwrap();
    let username = format!("testuser-{}", Uuid::new_v4());
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO users (username, password_hash, auth_provider, email) \
         VALUES ($1, $2, 'local', $3) RETURNING id",
    )
    .bind(&username)
    .bind(&hash)
    .bind(email)
    .fetch_one(&app.pool)
    .await
    .unwrap();

    let token = api::auth::jwt::encode_access_token(
        row.0,
        "user",
        "test-jwt-secret-at-least-32-bytes-long",
        3600,
    )
    .unwrap();

    (row.0, token)
}

async fn create_direct_share(
    app: &common::TestApp,
    token: &str,
    friend_email: &str,
    data_types: &[&str],
) -> serde_json::Value {
    let body = json!({
        "friend_email": friend_email,
        "data_types": data_types,
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/friends/shares",
            token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "create_direct_share failed");
    common::body_json(resp).await
}

async fn create_link_share(
    app: &common::TestApp,
    token: &str,
    data_types: &[&str],
) -> serde_json::Value {
    let body = json!({
        "data_types": data_types,
    });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/friends/shares",
            token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "create_link_share failed");
    common::body_json(resp).await
}

// =============================================================================
// Fix 1: accept_share auth bypass — removed OR friend_id IS NULL
// =============================================================================

#[tokio::test]
async fn accept_share_succeeds_for_invited_friend() {
    let app = common::setup().await;
    let friend_email = unique_email("friend");
    let (_owner_id, owner_token) =
        create_test_user_with_email(&app, &unique_email("owner")).await;
    let (_friend_id, friend_token) =
        create_test_user_with_email(&app, &friend_email).await;

    let share = create_direct_share(&app, &owner_token, &friend_email, &["checkins"]).await;
    let share_id = share["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/friends/shares/{share_id}/accept"),
            &friend_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn accept_share_rejects_uninvited_user() {
    let app = common::setup().await;
    let friend_email = unique_email("friend");
    let (_owner_id, owner_token) =
        create_test_user_with_email(&app, &unique_email("owner")).await;
    let (_friend_id, _friend_token) =
        create_test_user_with_email(&app, &friend_email).await;
    let (_stranger_id, stranger_token) =
        create_test_user_with_email(&app, &unique_email("stranger")).await;

    let share = create_direct_share(&app, &owner_token, &friend_email, &["checkins"]).await;
    let share_id = share["id"].as_str().unwrap();

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/friends/shares/{share_id}/accept"),
            &stranger_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn accept_share_rejects_for_link_share() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_test_user_with_email(&app, &unique_email("owner")).await;
    let (_user_id, user_token) =
        create_test_user_with_email(&app, &unique_email("user")).await;

    // Create a link share (no friend_email → friend_id is NULL)
    let share = create_link_share(&app, &owner_token, &["checkins"]).await;
    let share_id = share["id"].as_str().unwrap();

    // Try to accept via the direct accept endpoint — should fail because
    // friend_id IS NULL doesn't match anymore (this is the regression test)
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/friends/shares/{share_id}/accept"),
            &user_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}

// =============================================================================
// Fix 2: declined vs revoked distinction
// =============================================================================

#[tokio::test]
async fn revoke_by_owner_sets_revoked_status() {
    let app = common::setup().await;
    let friend_email = unique_email("friend");
    let (_owner_id, owner_token) =
        create_test_user_with_email(&app, &unique_email("owner")).await;
    let (_friend_id, friend_token) =
        create_test_user_with_email(&app, &friend_email).await;

    let share = create_direct_share(&app, &owner_token, &friend_email, &["checkins"]).await;
    let share_id = share["id"].as_str().unwrap();

    // Friend accepts
    app.app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/friends/shares/{share_id}/accept"),
            &friend_token,
            None,
        ))
        .await
        .unwrap();

    // Owner revokes
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/friends/shares/{share_id}"),
            &owner_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify status in DB
    let row: (String,) =
        sqlx::query_as("SELECT status FROM friend_shares WHERE id = $1")
            .bind(Uuid::parse_str(share_id).unwrap())
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(row.0, "revoked");
}

#[tokio::test]
async fn revoke_by_friend_sets_declined_status() {
    let app = common::setup().await;
    let friend_email = unique_email("friend");
    let (_owner_id, owner_token) =
        create_test_user_with_email(&app, &unique_email("owner")).await;
    let (_friend_id, friend_token) =
        create_test_user_with_email(&app, &friend_email).await;

    let share = create_direct_share(&app, &owner_token, &friend_email, &["checkins"]).await;
    let share_id = share["id"].as_str().unwrap();

    // Friend accepts
    app.app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/friends/shares/{share_id}/accept"),
            &friend_token,
            None,
        ))
        .await
        .unwrap();

    // Friend declines (calls DELETE)
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/friends/shares/{share_id}"),
            &friend_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify status in DB
    let row: (String,) =
        sqlx::query_as("SELECT status FROM friend_shares WHERE id = $1")
            .bind(Uuid::parse_str(share_id).unwrap())
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(row.0, "declined");
}

#[tokio::test]
async fn revoke_rejects_unrelated_user() {
    let app = common::setup().await;
    let friend_email = unique_email("friend");
    let (_owner_id, owner_token) =
        create_test_user_with_email(&app, &unique_email("owner")).await;
    let (_friend_id, friend_token) =
        create_test_user_with_email(&app, &friend_email).await;
    let (_stranger_id, stranger_token) =
        create_test_user_with_email(&app, &unique_email("stranger")).await;

    let share = create_direct_share(&app, &owner_token, &friend_email, &["checkins"]).await;
    let share_id = share["id"].as_str().unwrap();

    // Friend accepts
    app.app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            &format!("/api/v1/friends/shares/{share_id}/accept"),
            &friend_token,
            None,
        ))
        .await
        .unwrap();

    // Stranger tries to revoke
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            &format!("/api/v1/friends/shares/{share_id}"),
            &stranger_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}

// =============================================================================
// Fix 3: invite_token leak prevention
// =============================================================================

#[tokio::test]
async fn list_incoming_strips_invite_token() {
    let app = common::setup().await;
    let friend_email = unique_email("friend");
    let (_owner_id, owner_token) =
        create_test_user_with_email(&app, &unique_email("owner")).await;
    let (_friend_id, friend_token) =
        create_test_user_with_email(&app, &friend_email).await;

    create_direct_share(&app, &owner_token, &friend_email, &["checkins"]).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/friends/shares/incoming",
            &friend_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let json = common::body_json(resp).await;
    let shares = json.as_array().unwrap();
    assert_eq!(shares.len(), 1);
    assert!(shares[0]["invite_token"].is_null());
}

#[tokio::test]
async fn list_outgoing_preserves_invite_token_for_owner() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_test_user_with_email(&app, &unique_email("owner")).await;

    create_link_share(&app, &owner_token, &["checkins"]).await;

    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/friends/shares/outgoing",
            &owner_token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let json = common::body_json(resp).await;
    let shares = json.as_array().unwrap();
    assert_eq!(shares.len(), 1);
    assert!(!shares[0]["invite_token"].is_null());
}

// =============================================================================
// Fix 4: accept_by_token NULLs invite_token after acceptance
// =============================================================================

#[tokio::test]
async fn accept_by_token_nulls_invite_token() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_test_user_with_email(&app, &unique_email("owner")).await;
    let (_friend_id, friend_token) =
        create_test_user_with_email(&app, &unique_email("friend")).await;

    let share = create_link_share(&app, &owner_token, &["checkins"]).await;
    let token = share["invite_token"].as_str().unwrap().to_string();
    let share_id = share["id"].as_str().unwrap();

    // Accept via token endpoint
    let body = json!({ "token": token });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/friends/shares/accept-link",
            &friend_token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify invite_token and invite_expires_at are NULL in the DB
    let row: (Option<String>, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "SELECT invite_token, invite_expires_at FROM friend_shares WHERE id = $1",
    )
    .bind(Uuid::parse_str(share_id).unwrap())
    .fetch_one(&app.pool)
    .await
    .unwrap();
    assert!(row.0.is_none(), "invite_token should be NULL after acceptance");
    assert!(row.1.is_none(), "invite_expires_at should be NULL after acceptance");
}

#[tokio::test]
async fn accept_by_token_rejects_reused_token() {
    let app = common::setup().await;
    let (_owner_id, owner_token) =
        create_test_user_with_email(&app, &unique_email("owner")).await;
    let (_friend_id, friend_token) =
        create_test_user_with_email(&app, &unique_email("friend")).await;
    let (_other_id, other_token) =
        create_test_user_with_email(&app, &unique_email("other")).await;

    let share = create_link_share(&app, &owner_token, &["checkins"]).await;
    let token = share["invite_token"].as_str().unwrap().to_string();

    // First user accepts — should succeed
    let body = json!({ "token": token });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/friends/shares/accept-link",
            &friend_token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Second user tries the same token — should fail (token is NULLed)
    let body = json!({ "token": token });
    let resp = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "POST",
            "/api/v1/friends/shares/accept-link",
            &other_token,
            Some(&body),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}
