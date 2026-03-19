// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use crate::models::user::UserRow;
use sqlx::PgPool;
use uuid::Uuid;

/// Find a user by primary key.
pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, username, password_hash, auth_provider, email,
                data_region, federation_id, created_at
         FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
}

/// Find a user by username.
pub async fn find_by_username(pool: &PgPool, username: &str) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, username, password_hash, auth_provider, email,
                data_region, federation_id, created_at
         FROM users WHERE username = $1",
    )
    .bind(username)
    .fetch_one(pool)
    .await
}

/// Look up a Google-authenticated user by email, creating one if none exists.
pub async fn find_or_create_google_user(
    pool: &PgPool,
    email: &str,
    username: &str,
) -> Result<UserRow, sqlx::Error> {
    let existing = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, password_hash, auth_provider, email,
                data_region, federation_id, created_at
         FROM users WHERE email = $1 AND auth_provider = 'google'",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    if let Some(user) = existing {
        return Ok(user);
    }

    sqlx::query_as::<_, UserRow>(
        "INSERT INTO users (username, auth_provider, email)
         VALUES ($1, 'google', $2)
         RETURNING id, username, password_hash, auth_provider, email,
                   data_region, federation_id, created_at",
    )
    .bind(username)
    .bind(email)
    .fetch_one(pool)
    .await
}

/// Delete a user and all their data from child tables, then the user row.
pub async fn delete_user(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

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
