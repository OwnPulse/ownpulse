// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use serde_json::{Value, json};
use tower::ServiceExt;

use crate::common;

// ─── GET /api/v1/config (unauthenticated) ─────────────────────────────────────

#[tokio::test]
async fn test_config_returns_feature_flags_and_ios() {
    let app = common::setup().await;

    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/api/v1/config")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    // Check Cache-Control header
    let cache_control = response
        .headers()
        .get("cache-control")
        .expect("missing cache-control header")
        .to_str()
        .unwrap();
    assert_eq!(cache_control, "public, max-age=60");

    let body = common::body_json(response).await;
    assert!(body["feature_flags"].is_object());
    assert!(body["ios"].is_object());
    // With no flags in DB, feature_flags should be empty
    assert_eq!(body["feature_flags"].as_object().unwrap().len(), 0);
    assert!(body["ios"]["min_supported_version"].is_null());
    assert!(body["ios"]["force_upgrade_below"].is_null());
}

#[tokio::test]
async fn test_config_with_ios_values() {
    let app = common::setup_with_config(|cfg| {
        cfg.ios_min_version = Some("2.0.0".to_string());
        cfg.ios_force_upgrade_below = Some("1.5.0".to_string());
    })
    .await;

    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/api/v1/config")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    assert_eq!(body["ios"]["min_supported_version"], "2.0.0");
    assert_eq!(body["ios"]["force_upgrade_below"], "1.5.0");
}

// ─── Admin feature flags: auth ────────────────────────────────────────────────

#[tokio::test]
async fn test_list_feature_flags_requires_auth() {
    let app = common::setup().await;

    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/api/v1/admin/feature-flags")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_list_feature_flags_requires_admin() {
    let app = common::setup().await;
    let (_user_id, user_token) = common::create_test_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/admin/feature-flags",
            &user_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 403);
}

// ─── Admin CRUD ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_upsert_creates_new_flag() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            "/api/v1/admin/feature-flags/dark_mode",
            &admin_token,
            Some(&json!({"enabled": true, "description": "Enable dark mode"})),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    assert_eq!(body["key"], "dark_mode");
    assert_eq!(body["enabled"], true);
    assert_eq!(body["description"], "Enable dark mode");
    assert!(body["id"].is_string());
    assert!(body["created_at"].is_string());
    assert!(body["updated_at"].is_string());
}

#[tokio::test]
async fn test_upsert_updates_existing_flag() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    // Create
    let _response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            "/api/v1/admin/feature-flags/beta",
            &admin_token,
            Some(&json!({"enabled": false, "description": "Beta features"})),
        ))
        .await
        .unwrap();

    // Update
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            "/api/v1/admin/feature-flags/beta",
            &admin_token,
            Some(&json!({"enabled": true, "description": "Beta features (now live)"})),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    assert_eq!(body["key"], "beta");
    assert_eq!(body["enabled"], true);
    assert_eq!(body["description"], "Beta features (now live)");
}


#[tokio::test]
async fn test_list_flags_includes_created() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    // Create two flags
    app.app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            "/api/v1/admin/feature-flags/alpha",
            &admin_token,
            Some(&json!({"enabled": true})),
        ))
        .await
        .unwrap();

    app.app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            "/api/v1/admin/feature-flags/beta",
            &admin_token,
            Some(&json!({"enabled": false, "description": "Beta"})),
        ))
        .await
        .unwrap();

    // List
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/admin/feature-flags",
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    let flags = body.as_array().unwrap();
    assert_eq!(flags.len(), 2);

    // Ordered by key
    assert_eq!(flags[0]["key"], "alpha");
    assert_eq!(flags[1]["key"], "beta");
}

#[tokio::test]
async fn test_delete_flag() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    // Create
    app.app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            "/api/v1/admin/feature-flags/temp",
            &admin_token,
            Some(&json!({"enabled": true})),
        ))
        .await
        .unwrap();

    // Delete
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            "/api/v1/admin/feature-flags/temp",
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 204);

    // Verify gone
    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "GET",
            "/api/v1/admin/feature-flags",
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    let body = common::body_json(response).await;
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_delete_nonexistent_flag_returns_404() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    let response = app
        .app
        .clone()
        .oneshot(common::auth_request(
            "DELETE",
            "/api/v1/admin/feature-flags/nonexistent",
            &admin_token,
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

// ─── Config reflects flag changes ─────────────────────────────────────────────

#[tokio::test]
async fn test_config_reflects_flag_changes() {
    let app = common::setup().await;
    let (_admin_id, admin_token) = common::create_admin_user(&app).await;

    // Create a flag
    app.app
        .clone()
        .oneshot(common::auth_request(
            "PUT",
            "/api/v1/admin/feature-flags/new_dashboard",
            &admin_token,
            Some(&json!({"enabled": true})),
        ))
        .await
        .unwrap();

    // Check config
    let response = app
        .app
        .clone()
        .oneshot(
            http::Request::builder()
                .method("GET")
                .uri("/api/v1/config")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = common::body_json(response).await;
    let flags = body["feature_flags"].as_object().unwrap();
    assert_eq!(flags.get("new_dashboard"), Some(&Value::Bool(true)));
}
