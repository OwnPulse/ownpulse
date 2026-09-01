// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Server-side state for browser-redirect OAuth connect flows — see the
//! `oauth_states` migration for why this exists instead of a cookie.

use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// How long a state row is honored after creation. Well over the time a
/// user takes to complete Google's consent screen, short enough that a
/// leftover row from an abandoned flow is not usable for long.
pub const STATE_TTL_MINUTES: i64 = 10;

/// Record that `user_id` started a `provider` connect flow, keyed by the
/// CSRF `state` value handed to the provider.
pub async fn insert(
    pool: &PgPool,
    state: Uuid,
    user_id: Uuid,
    provider: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO oauth_states (state, user_id, provider) VALUES ($1, $2, $3)")
        .bind(state)
        .bind(user_id)
        .bind(provider)
        .execute(pool)
        .await?;
    Ok(())
}

/// Consume a state row: deletes it (single-use — a replayed callback finds
/// nothing) and returns the user id that started the flow, if the row
/// existed, matched `provider`, and was created within [`STATE_TTL_MINUTES`].
pub async fn consume(
    pool: &PgPool,
    state: Uuid,
    provider: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row: Option<(Uuid, DateTime<Utc>)> = sqlx::query_as(
        "DELETE FROM oauth_states WHERE state = $1 AND provider = $2 RETURNING user_id, created_at",
    )
    .bind(state)
    .bind(provider)
    .fetch_optional(pool)
    .await?;

    Ok(row.and_then(|(user_id, created_at)| {
        if Utc::now() - created_at <= Duration::minutes(STATE_TTL_MINUTES) {
            Some(user_id)
        } else {
            None
        }
    }))
}
