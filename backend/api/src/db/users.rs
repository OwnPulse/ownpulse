// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use crate::models::user::UserRow;
use sqlx::PgPool;
use uuid::Uuid;

/// Find a user by primary key.
pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, username, password_hash, auth_provider, email,
                role, status, data_region, federation_id, created_at
         FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
}

/// Find a user by email address.
pub async fn find_by_email(pool: &PgPool, email: &str) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, username, password_hash, auth_provider, email,
                role, status, data_region, federation_id, created_at
         FROM users WHERE LOWER(email) = LOWER($1)",
    )
    .bind(email)
    .fetch_one(pool)
    .await
}

/// Look up a Google-authenticated user by email, creating one if none exists.
///
/// `display_name` is an optional human-readable name derived from the email
/// local part; it is stored in the nullable `username` column.
pub async fn find_or_create_google_user(
    pool: &PgPool,
    email: &str,
    display_name: Option<&str>,
) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "INSERT INTO users (email, username, auth_provider)
         VALUES (LOWER($1), $2, 'google')
         ON CONFLICT (LOWER(email)) DO UPDATE SET email = EXCLUDED.email
         RETURNING *",
    )
    .bind(email)
    .bind(display_name)
    .fetch_one(pool)
    .await
}

/// List all users ordered by creation date.
pub async fn list_all_users(pool: &PgPool) -> Result<Vec<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, username, password_hash, auth_provider, email,
                role, status, data_region, federation_id, created_at
         FROM users ORDER BY created_at",
    )
    .fetch_all(pool)
    .await
}

/// Update a user's role.
pub async fn update_user_role(
    pool: &PgPool,
    user_id: Uuid,
    role: &str,
) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "UPDATE users SET role = $2 WHERE id = $1
         RETURNING id, username, password_hash, auth_provider, email,
                   role, status, data_region, federation_id, created_at",
    )
    .bind(user_id)
    .bind(role)
    .fetch_one(pool)
    .await
}

/// Delete a user and all their data from child tables, then the user row.
pub async fn delete_user(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM friend_shares WHERE owner_id = $1 OR friend_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM export_jobs WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM sharing_consents WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM integration_tokens WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM healthkit_write_queue WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM source_preferences WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM observations WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM calendar_days WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    // lab_results has FK to uploaded_files; delete lab_results first
    sqlx::query("DELETE FROM lab_results WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM genetic_records WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM uploaded_files WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM daily_checkins WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM interventions WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    // health_records has self-referential duplicate_of FK; clear references first
    sqlx::query("UPDATE health_records SET duplicate_of = NULL WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM health_records WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await
}
