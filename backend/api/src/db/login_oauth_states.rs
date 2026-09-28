// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Server-side state for the Google *login* redirect flow — see the
//! `login_oauth_states` migration for why the callback reads platform,
//! invite code and link target from a row instead of from cookies.

use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::oauth_states::STATE_TTL_MINUTES;

/// What `google_login` recorded about the flow, recovered by the callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginOAuthState {
    /// The flow was started by the iOS app (`?platform=ios`), so the callback
    /// redirects to the `ownpulse://` scheme instead of setting cookies.
    pub is_native: bool,
    /// Invite code supplied at initiation, for registering a new user.
    pub invite_code: Option<String>,
    /// `Some` for link mode, bound to the user that was authenticated when
    /// the flow started. `None` for login/register.
    pub link_user_id: Option<Uuid>,
}

/// Record a started Google login flow, keyed by the CSRF `state` value
/// handed to Google.
pub async fn insert(
    pool: &PgPool,
    state: Uuid,
    is_native: bool,
    invite_code: Option<&str>,
    link_user_id: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO login_oauth_states (state, is_native, invite_code, link_user_id)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(state)
    .bind(is_native)
    .bind(invite_code)
    .bind(link_user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Consume a state row: deletes it (single-use — a replayed callback finds
/// nothing) and returns what the initiation recorded, if the row existed and
/// was created within [`STATE_TTL_MINUTES`].
pub async fn consume(pool: &PgPool, state: Uuid) -> Result<Option<LoginOAuthState>, sqlx::Error> {
    /// Deleted row, before the TTL check drops `created_at`.
    #[derive(sqlx::FromRow)]
    struct ConsumedRow {
        is_native: bool,
        invite_code: Option<String>,
        link_user_id: Option<Uuid>,
        created_at: DateTime<Utc>,
    }

    let row: Option<ConsumedRow> = sqlx::query_as(
        "DELETE FROM login_oauth_states WHERE state = $1
         RETURNING is_native, invite_code, link_user_id, created_at",
    )
    .bind(state)
    .fetch_optional(pool)
    .await?;

    Ok(row.and_then(|row| {
        if Utc::now() - row.created_at <= Duration::minutes(STATE_TTL_MINUTES) {
            Some(LoginOAuthState {
                is_native: row.is_native,
                invite_code: row.invite_code,
                link_user_id: row.link_user_id,
            })
        } else {
            None
        }
    }))
}

/// Delete rows past their TTL. Only a flow the user abandoned (closed the
/// Google consent screen) leaves one behind — a completed callback deletes
/// its own. Swept daily so those don't accumulate.
pub async fn delete_expired(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM login_oauth_states WHERE created_at < now() - make_interval(mins => $1)",
    )
    .bind(STATE_TTL_MINUTES as i32)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
