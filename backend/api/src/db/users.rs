// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use crate::models::user::UserRow;
use sqlx::PgPool;
use uuid::Uuid;

/// Returns true if no users exist in the database.
pub async fn is_empty(pool: &PgPool) -> Result<bool, sqlx::Error> {
    let (exists,): (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM users)")
        .fetch_one(pool)
        .await?;
    Ok(!exists)
}

/// Returns true if no users exist in the database (transaction-aware variant).
pub async fn is_empty_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<bool, sqlx::Error> {
    let (exists,): (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM users)")
        .fetch_one(&mut **tx)
        .await?;
    Ok(!exists)
}

/// Find a user by primary key.
pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, username, password_hash, auth_provider, email,
                role, data_region, federation_id, status, created_at
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
                role, data_region, federation_id, status, created_at
         FROM users WHERE LOWER(email) = LOWER($1)",
    )
    .bind(email)
    .fetch_one(pool)
    .await
}

/// Look up a Google-authenticated user, creating one if none exists.
///
/// `display_name` is an optional human-readable name derived from the email
/// local part; it is stored in the nullable `username` column.
///
/// Lookup order:
/// 1. `user_auth_methods` by `(provider='google', provider_subject=google_sub)`
/// 2. `user_auth_methods` by `(provider='google', email=email)` (legacy rows without sub)
/// 3. Create a new user and insert an auth method row.
pub async fn find_or_create_google_user(
    pool: &PgPool,
    google_sub: &str,
    email: &str,
    display_name: Option<&str>,
) -> Result<UserRow, sqlx::Error> {
    // 1. Look up by stable subject (preferred — doesn't change if user changes email)
    match crate::db::user_auth_methods::find_by_provider_subject(pool, "google", google_sub).await {
        Ok(user) => return Ok(user),
        Err(sqlx::Error::RowNotFound) => {}
        Err(e) => return Err(e),
    }

    // 2. Look up by email (handles migrated rows that have no subject yet)
    match crate::db::user_auth_methods::find_by_provider_email(pool, "google", email).await {
        Ok(user) => {
            // Backfill the missing provider_subject so future lookups use the faster path.
            sqlx::query(
                "UPDATE user_auth_methods SET provider_subject = $1
                 WHERE user_id = $2 AND provider = 'google' AND provider_subject IS NULL",
            )
            .bind(google_sub)
            .bind(user.id)
            .execute(pool)
            .await?;
            return Ok(user);
        }
        Err(sqlx::Error::RowNotFound) => {}
        Err(e) => return Err(e),
    }

    // 3. Create a new user and auth method row inside a transaction.
    let mut tx = pool.begin().await?;

    let user = sqlx::query_as::<_, UserRow>(
        "INSERT INTO users (username, email, auth_provider)
         VALUES ($1, $2, 'google')
         RETURNING id, username, password_hash, auth_provider, email,
                   role, data_region, federation_id, status, created_at",
    )
    .bind(display_name)
    .bind(email)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject, email)
         VALUES ($1, 'google', $2, $3)",
    )
    .bind(user.id)
    .bind(google_sub)
    .bind(email)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(user)
}

/// Look up an existing Google user inside a transaction (read-only).
///
/// Returns `Err(sqlx::Error::RowNotFound)` when no matching user is found,
/// allowing the caller to decide whether to create a new one.
pub async fn find_google_user_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    google_sub: &str,
    email: &str,
) -> Result<UserRow, sqlx::Error> {
    // 1. Look up by stable subject
    let existing = sqlx::query_as::<_, UserRow>(
        "SELECT u.id, u.username, u.password_hash, u.auth_provider, u.email,
                u.role, u.data_region, u.federation_id, u.status, u.created_at
         FROM users u
         JOIN user_auth_methods m ON m.user_id = u.id
         WHERE m.provider = 'google' AND m.provider_subject = $1",
    )
    .bind(google_sub)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(user) = existing {
        return Ok(user);
    }

    // 2. Look up by email (handles migrated rows)
    let existing_by_email = sqlx::query_as::<_, UserRow>(
        "SELECT u.id, u.username, u.password_hash, u.auth_provider, u.email,
                u.role, u.data_region, u.federation_id, u.status, u.created_at
         FROM users u
         JOIN user_auth_methods m ON m.user_id = u.id
         WHERE m.provider = 'google' AND m.email = $1",
    )
    .bind(email)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(user) = existing_by_email {
        // Backfill subject
        sqlx::query(
            "UPDATE user_auth_methods SET provider_subject = $1
             WHERE user_id = $2 AND provider = 'google' AND provider_subject IS NULL",
        )
        .bind(google_sub)
        .bind(user.id)
        .execute(&mut **tx)
        .await?;
        return Ok(user);
    }

    Err(sqlx::Error::RowNotFound)
}

/// Look up or create a Google user inside an existing transaction.
///
/// Used when the invite claim and user creation must be atomic.
/// Lookup follows the same order as [`find_or_create_google_user`].
pub async fn find_or_create_google_user_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    google_sub: &str,
    email: &str,
    display_name: Option<&str>,
) -> Result<UserRow, sqlx::Error> {
    // 1. Look up by stable subject
    let existing = sqlx::query_as::<_, UserRow>(
        "SELECT u.id, u.username, u.password_hash, u.auth_provider, u.email,
                u.role, u.data_region, u.federation_id, u.status, u.created_at
         FROM users u
         JOIN user_auth_methods m ON m.user_id = u.id
         WHERE m.provider = 'google' AND m.provider_subject = $1",
    )
    .bind(google_sub)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(user) = existing {
        return Ok(user);
    }

    // 2. Look up by email (handles migrated rows)
    let existing_by_email = sqlx::query_as::<_, UserRow>(
        "SELECT u.id, u.username, u.password_hash, u.auth_provider, u.email,
                u.role, u.data_region, u.federation_id, u.status, u.created_at
         FROM users u
         JOIN user_auth_methods m ON m.user_id = u.id
         WHERE m.provider = 'google' AND m.email = $1",
    )
    .bind(email)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(user) = existing_by_email {
        // Backfill subject
        sqlx::query(
            "UPDATE user_auth_methods SET provider_subject = $1
             WHERE user_id = $2 AND provider = 'google' AND provider_subject IS NULL",
        )
        .bind(google_sub)
        .bind(user.id)
        .execute(&mut **tx)
        .await?;
        return Ok(user);
    }

    // 3. Create new user and auth method row.
    let user = sqlx::query_as::<_, UserRow>(
        "INSERT INTO users (username, email, auth_provider)
         VALUES ($1, $2, 'google')
         RETURNING id, username, password_hash, auth_provider, email,
                   role, data_region, federation_id, status, created_at",
    )
    .bind(display_name)
    .bind(email)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject, email)
         VALUES ($1, 'google', $2, $3)",
    )
    .bind(user.id)
    .bind(google_sub)
    .bind(email)
    .execute(&mut **tx)
    .await?;

    Ok(user)
}

/// Look up an Apple-authenticated user, creating one if none exists.
///
/// Lookup order:
/// 1. `user_auth_methods` by `(provider='apple', provider_subject=apple_sub)`
/// 2. `user_auth_methods` by `(provider='apple', email=email)` if email is present
/// 3. Create a new user and insert an auth method row.
pub async fn find_or_create_apple_user(
    pool: &PgPool,
    apple_sub: &str,
    email: Option<&str>,
    username: &str,
) -> Result<UserRow, sqlx::Error> {
    // 1. Look up by Apple subject
    match crate::db::user_auth_methods::find_by_provider_subject(pool, "apple", apple_sub).await {
        Ok(user) => return Ok(user),
        Err(sqlx::Error::RowNotFound) => {}
        Err(e) => return Err(e),
    }

    // 2. Look up by email (only when Apple provided one)
    if let Some(em) = email {
        match crate::db::user_auth_methods::find_by_provider_email(pool, "apple", em).await {
            Ok(user) => {
                // Backfill subject
                sqlx::query(
                    "UPDATE user_auth_methods SET provider_subject = $1
                     WHERE user_id = $2 AND provider = 'apple' AND provider_subject IS NULL",
                )
                .bind(apple_sub)
                .bind(user.id)
                .execute(pool)
                .await?;
                return Ok(user);
            }
            Err(sqlx::Error::RowNotFound) => {}
            Err(e) => return Err(e),
        }
    }

    // 3. Create new user and auth method row.
    let mut tx = pool.begin().await?;

    let user = sqlx::query_as::<_, UserRow>(
        "INSERT INTO users (username, email, auth_provider)
         VALUES ($1, $2, 'apple')
         RETURNING id, username, password_hash, auth_provider, email,
                   role, data_region, federation_id, status, created_at",
    )
    .bind(username)
    .bind(email)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject, email)
         VALUES ($1, 'apple', $2, $3)",
    )
    .bind(user.id)
    .bind(apple_sub)
    .bind(email)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(user)
}

/// Look up an existing Apple user inside a transaction (read-only).
///
/// Returns `Err(sqlx::Error::RowNotFound)` when no matching user is found,
/// allowing the caller to decide whether to create a new one.
pub async fn find_apple_user_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    apple_sub: &str,
    email: Option<&str>,
) -> Result<UserRow, sqlx::Error> {
    // 1. Look up by Apple subject
    let existing = sqlx::query_as::<_, UserRow>(
        "SELECT u.id, u.username, u.password_hash, u.auth_provider, u.email,
                u.role, u.data_region, u.federation_id, u.status, u.created_at
         FROM users u
         JOIN user_auth_methods m ON m.user_id = u.id
         WHERE m.provider = 'apple' AND m.provider_subject = $1",
    )
    .bind(apple_sub)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(user) = existing {
        return Ok(user);
    }

    // 2. Look up by email (only when Apple provided one)
    if let Some(em) = email {
        let existing_by_email = sqlx::query_as::<_, UserRow>(
            "SELECT u.id, u.username, u.password_hash, u.auth_provider, u.email,
                    u.role, u.data_region, u.federation_id, u.status, u.created_at
             FROM users u
             JOIN user_auth_methods m ON m.user_id = u.id
             WHERE m.provider = 'apple' AND m.email = $1",
        )
        .bind(em)
        .fetch_optional(&mut **tx)
        .await?;

        if let Some(user) = existing_by_email {
            // Backfill subject
            sqlx::query(
                "UPDATE user_auth_methods SET provider_subject = $1
                 WHERE user_id = $2 AND provider = 'apple' AND provider_subject IS NULL",
            )
            .bind(apple_sub)
            .bind(user.id)
            .execute(&mut **tx)
            .await?;
            return Ok(user);
        }
    }

    Err(sqlx::Error::RowNotFound)
}

/// Look up or create an Apple user inside an existing transaction.
///
/// Used when the invite claim and user creation must be atomic.
/// Lookup follows the same order as [`find_or_create_apple_user`].
pub async fn find_or_create_apple_user_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    apple_sub: &str,
    email: Option<&str>,
    username: &str,
) -> Result<UserRow, sqlx::Error> {
    // 1. Look up by Apple subject
    let existing = sqlx::query_as::<_, UserRow>(
        "SELECT u.id, u.username, u.password_hash, u.auth_provider, u.email,
                u.role, u.data_region, u.federation_id, u.status, u.created_at
         FROM users u
         JOIN user_auth_methods m ON m.user_id = u.id
         WHERE m.provider = 'apple' AND m.provider_subject = $1",
    )
    .bind(apple_sub)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(user) = existing {
        return Ok(user);
    }

    // 2. Look up by email (only when Apple provided one)
    if let Some(em) = email {
        let existing_by_email = sqlx::query_as::<_, UserRow>(
            "SELECT u.id, u.username, u.password_hash, u.auth_provider, u.email,
                    u.role, u.data_region, u.federation_id, u.status, u.created_at
             FROM users u
             JOIN user_auth_methods m ON m.user_id = u.id
             WHERE m.provider = 'apple' AND m.email = $1",
        )
        .bind(em)
        .fetch_optional(&mut **tx)
        .await?;

        if let Some(user) = existing_by_email {
            // Backfill subject
            sqlx::query(
                "UPDATE user_auth_methods SET provider_subject = $1
                 WHERE user_id = $2 AND provider = 'apple' AND provider_subject IS NULL",
            )
            .bind(apple_sub)
            .bind(user.id)
            .execute(&mut **tx)
            .await?;
            return Ok(user);
        }
    }

    // 3. Create new user and auth method row.
    let user = sqlx::query_as::<_, UserRow>(
        "INSERT INTO users (username, email, auth_provider)
         VALUES ($1, $2, 'apple')
         RETURNING id, username, password_hash, auth_provider, email,
                   role, data_region, federation_id, status, created_at",
    )
    .bind(username)
    .bind(email)
    .fetch_one(&mut **tx)
    .await?;

    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject, email)
         VALUES ($1, 'apple', $2, $3)",
    )
    .bind(user.id)
    .bind(apple_sub)
    .bind(email)
    .execute(&mut **tx)
    .await?;

    Ok(user)
}

/// Check whether any user with the given email exists (inside a transaction).
///
/// Used during OAuth registration to detect email collisions before creating
/// a new user, so the caller can redirect with a descriptive error rather
/// than relying on a unique-constraint violation.
pub async fn email_exists_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    email: &str,
) -> Result<bool, sqlx::Error> {
    let row: Option<(i32,)> =
        sqlx::query_as("SELECT 1 AS one FROM users WHERE LOWER(email) = LOWER($1)")
            .bind(email)
            .fetch_optional(&mut **tx)
            .await?;
    Ok(row.is_some())
}

/// List all users ordered by creation date.
pub async fn list_all_users(pool: &PgPool) -> Result<Vec<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "SELECT id, username, password_hash, auth_provider, email,
                role, data_region, federation_id, status, created_at
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
                   role, data_region, federation_id, status, created_at",
    )
    .bind(user_id)
    .bind(role)
    .fetch_one(pool)
    .await
}

/// Create a new user with an email and bcrypt-hashed password.
pub async fn create_user_with_password(
    pool: &PgPool,
    email: &str,
    username: Option<&str>,
    password_hash: &str,
) -> Result<UserRow, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let user = sqlx::query_as::<_, UserRow>(
        "INSERT INTO users (email, username, password_hash, auth_provider)
         VALUES ($1, $2, $3, 'local')
         RETURNING id, username, password_hash, auth_provider, email,
                   role, data_region, federation_id, status, created_at",
    )
    .bind(email)
    .bind(username)
    .bind(password_hash)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO user_auth_methods (user_id, provider, provider_subject, email)
         VALUES ($1, 'local', $2, $3)",
    )
    .bind(user.id)
    .bind(user.id.to_string())
    .bind(email)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(user)
}

/// Update a user's status (active/disabled).
pub async fn update_user_status(
    pool: &PgPool,
    user_id: Uuid,
    status: &str,
) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(
        "UPDATE users SET status = $2 WHERE id = $1
         RETURNING id, username, password_hash, auth_provider, email,
                   role, data_region, federation_id, status, created_at",
    )
    .bind(user_id)
    .bind(status)
    .fetch_one(pool)
    .await
}

/// Delete a user and all their data from child tables, then the user row.
pub async fn delete_user(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM invite_claims WHERE invite_code_id IN (SELECT id FROM invite_codes WHERE created_by = $1)")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM invite_codes WHERE created_by = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
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
    sqlx::query("DELETE FROM user_auth_methods WHERE user_id = $1")
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
