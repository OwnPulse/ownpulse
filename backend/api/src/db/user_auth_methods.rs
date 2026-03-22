// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Database access for the `user_auth_methods` junction table.

use crate::models::user::{AuthMethodRow, UserRow};
use sqlx::PgPool;
use uuid::Uuid;

/// Find a user by their provider + provider_subject (e.g. Google/Apple sub claim).
pub async fn find_by_provider_subject(
    pool: &PgPool,
    provider: &str,
    subject: &str,
) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT u.id, u.username, u.password_hash, u.auth_provider, u.email,
                u.role, u.data_region, u.federation_id, u.created_at
         FROM users u
         JOIN user_auth_methods m ON m.user_id = u.id
         WHERE m.provider = $1 AND m.provider_subject = $2",
    )
    .bind(provider)
    .bind(subject)
    .fetch_one(pool)
    .await
}

/// Find a user by their provider + email (fallback when subject is unknown).
pub async fn find_by_provider_email(
    pool: &PgPool,
    provider: &str,
    email: &str,
) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT u.id, u.username, u.password_hash, u.auth_provider, u.email,
                u.role, u.data_region, u.federation_id, u.created_at
         FROM users u
         JOIN user_auth_methods m ON m.user_id = u.id
         WHERE m.provider = $1 AND m.email = $2",
    )
    .bind(provider)
    .bind(email)
    .fetch_one(pool)
    .await
}

/// Insert a new auth method record for an existing user.
pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    provider: &str,
    provider_subject: Option<&str>,
    email: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject, email)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(provider)
    .bind(provider_subject)
    .bind(email)
    .execute(pool)
    .await?;
    Ok(())
}

/// List all auth methods linked to a user, ordered by creation date.
pub async fn list_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<AuthMethodRow>, sqlx::Error> {
    sqlx::query_as::<_, AuthMethodRow>(
        "SELECT id, provider, email, created_at
         FROM user_auth_methods
         WHERE user_id = $1
         ORDER BY created_at",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Delete the auth method for a specific provider from a user's account.
pub async fn delete(
    pool: &PgPool,
    user_id: Uuid,
    provider: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM user_auth_methods WHERE user_id = $1 AND provider = $2",
    )
    .bind(user_id)
    .bind(provider)
    .execute(pool)
    .await?;
    Ok(())
}

/// Count how many auth methods a user has linked.
pub async fn count_for_user(pool: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM user_auth_methods WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    Ok(row.0)
}
