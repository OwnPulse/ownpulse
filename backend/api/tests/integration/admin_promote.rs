// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use super::common;

/// The startup check should promote the first user to admin when no admin exists.
#[tokio::test]
async fn test_promote_first_user_to_admin() {
    let app = common::setup().await;

    // Insert a single user with role "user" (the default).
    let (user_id, _) = common::create_test_user(&app).await;

    // Verify the user starts as "user".
    let role: String =
        sqlx::query_scalar("SELECT role FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(&app.pool)
            .await
            .expect("user should exist");
    assert_eq!(role, "user");

    // Run the startup promotion check.
    let promoted = api::db::users::ensure_first_user_is_admin(&app.pool)
        .await
        .expect("ensure_first_user_is_admin should succeed");
    assert!(promoted, "first user should have been promoted");

    // Verify the user is now admin.
    let role: String =
        sqlx::query_scalar("SELECT role FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(&app.pool)
            .await
            .expect("user should exist");
    assert_eq!(role, "admin");
}

/// Calling the promotion a second time should be a no-op (idempotent).
#[tokio::test]
async fn test_promote_first_user_idempotent() {
    let app = common::setup().await;
    let (user_id, _) = common::create_test_user(&app).await;

    // First call promotes.
    let promoted = api::db::users::ensure_first_user_is_admin(&app.pool)
        .await
        .expect("first call should succeed");
    assert!(promoted);

    // Second call is a no-op because an admin already exists.
    let promoted = api::db::users::ensure_first_user_is_admin(&app.pool)
        .await
        .expect("second call should succeed");
    assert!(!promoted, "second call should not promote anyone");

    // Role is still admin.
    let role: String =
        sqlx::query_scalar("SELECT role FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(&app.pool)
            .await
            .expect("user should exist");
    assert_eq!(role, "admin");
}

/// When an admin already exists, no user should be promoted.
#[tokio::test]
async fn test_no_promotion_when_admin_exists() {
    let app = common::setup().await;

    // Create an admin user first.
    let (_admin_id, _) = common::create_admin_user(&app).await;

    // Create a regular user.
    let (user_id, _) = common::create_test_user(&app).await;

    let promoted = api::db::users::ensure_first_user_is_admin(&app.pool)
        .await
        .expect("should succeed");
    assert!(!promoted, "should not promote when admin already exists");

    // The regular user should still be "user".
    let role: String =
        sqlx::query_scalar("SELECT role FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(&app.pool)
            .await
            .expect("user should exist");
    assert_eq!(role, "user");
}

/// With no users at all, the function should be a no-op.
#[tokio::test]
async fn test_no_promotion_when_no_users() {
    let app = common::setup().await;

    let promoted = api::db::users::ensure_first_user_is_admin(&app.pool)
        .await
        .expect("should succeed on empty table");
    assert!(!promoted, "should not promote when no users exist");
}
