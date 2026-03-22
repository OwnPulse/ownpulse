// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::friend_share::{FriendShareResponse, FriendShareRow};

#[derive(sqlx::FromRow)]
struct ShareWithUsername {
    id: Uuid,
    owner_id: Uuid,
    friend_id: Option<Uuid>,
    status: String,
    invite_token: Option<String>,
    #[allow(dead_code)]
    invite_expires_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    accepted_at: Option<DateTime<Utc>>,
    #[allow(dead_code)]
    revoked_at: Option<DateTime<Utc>>,
    // joined field — owner or friend username depending on query direction
    peer_username: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PermissionRow {
    share_id: Uuid,
    data_type: String,
}

/// Create a new friend share record.
pub async fn create_share(
    pool: &PgPool,
    owner_id: Uuid,
    friend_id: Option<Uuid>,
    invite_token: Option<&str>,
    invite_expires_at: Option<DateTime<Utc>>,
) -> Result<FriendShareRow, sqlx::Error> {
    sqlx::query_as::<_, FriendShareRow>(
        "INSERT INTO friend_shares (owner_id, friend_id, status, invite_token, invite_expires_at)
         VALUES ($1, $2, 'pending', $3, $4)
         RETURNING id, owner_id, friend_id, status, invite_token,
                   invite_expires_at, created_at, accepted_at, revoked_at",
    )
    .bind(owner_id)
    .bind(friend_id)
    .bind(invite_token)
    .bind(invite_expires_at)
    .fetch_one(pool)
    .await
}

/// Replace all permissions for a share with the given data types.
pub async fn set_permissions(
    pool: &PgPool,
    share_id: Uuid,
    data_types: &[String],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM friend_share_permissions WHERE share_id = $1")
        .bind(share_id)
        .execute(&mut *tx)
        .await?;

    for dt in data_types {
        sqlx::query("INSERT INTO friend_share_permissions (share_id, data_type) VALUES ($1, $2)")
            .bind(share_id)
            .bind(dt)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await
}

/// Accept a pending share by share ID (direct share).
pub async fn accept_share(
    pool: &PgPool,
    share_id: Uuid,
    friend_id: Uuid,
) -> Result<(), sqlx::Error> {
    let result = sqlx::query(
        "UPDATE friend_shares
         SET status = 'accepted', friend_id = $2, accepted_at = now()
         WHERE id = $1
           AND (friend_id = $2 OR friend_id IS NULL)
           AND status = 'pending'",
    )
    .bind(share_id)
    .bind(friend_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(sqlx::Error::RowNotFound);
    }
    Ok(())
}

/// Accept a pending share via invite token.
pub async fn accept_by_token(
    pool: &PgPool,
    token: &str,
    friend_id: Uuid,
) -> Result<FriendShareRow, sqlx::Error> {
    sqlx::query_as::<_, FriendShareRow>(
        "UPDATE friend_shares
         SET status = 'accepted', friend_id = $2, accepted_at = now()
         WHERE invite_token = $1
           AND status = 'pending'
           AND (invite_expires_at IS NULL OR invite_expires_at > now())
           AND owner_id != $2
         RETURNING id, owner_id, friend_id, status, invite_token,
                   invite_expires_at, created_at, accepted_at, revoked_at",
    )
    .bind(token)
    .bind(friend_id)
    .fetch_one(pool)
    .await
}

/// Revoke or decline a share. Either party can revoke.
pub async fn revoke_share(
    pool: &PgPool,
    share_id: Uuid,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    let result = sqlx::query(
        "UPDATE friend_shares
         SET status = 'revoked', revoked_at = now()
         WHERE id = $1
           AND (owner_id = $2 OR friend_id = $2)
           AND status != 'revoked'",
    )
    .bind(share_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(sqlx::Error::RowNotFound);
    }
    Ok(())
}

/// List outgoing shares (where the user is the owner).
pub async fn list_outgoing(
    pool: &PgPool,
    owner_id: Uuid,
) -> Result<Vec<FriendShareResponse>, sqlx::Error> {
    let shares = sqlx::query_as::<_, ShareWithUsername>(
        "SELECT fs.id, fs.owner_id, fs.friend_id, fs.status, fs.invite_token,
                fs.invite_expires_at, fs.created_at, fs.accepted_at, fs.revoked_at,
                u.username AS peer_username
         FROM friend_shares fs
         LEFT JOIN users u ON u.id = fs.friend_id
         WHERE fs.owner_id = $1 AND fs.status != 'revoked'
         ORDER BY fs.created_at DESC",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await?;

    build_responses(pool, shares, true, owner_id).await
}

/// List incoming shares (where the user is the friend).
pub async fn list_incoming(
    pool: &PgPool,
    friend_id: Uuid,
) -> Result<Vec<FriendShareResponse>, sqlx::Error> {
    let shares = sqlx::query_as::<_, ShareWithUsername>(
        "SELECT fs.id, fs.owner_id, fs.friend_id, fs.status, fs.invite_token,
                fs.invite_expires_at, fs.created_at, fs.accepted_at, fs.revoked_at,
                u.username AS peer_username
         FROM friend_shares fs
         LEFT JOIN users u ON u.id = fs.owner_id
         WHERE fs.friend_id = $1 AND fs.status != 'revoked'
         ORDER BY fs.created_at DESC",
    )
    .bind(friend_id)
    .fetch_all(pool)
    .await?;

    build_responses(pool, shares, false, friend_id).await
}

/// Get the data types a friend is permitted to view for a given owner.
pub async fn get_permitted_types(
    pool: &PgPool,
    owner_id: Uuid,
    friend_id: Uuid,
) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query_as::<_, PermissionRow>(
        "SELECT fsp.share_id, fsp.data_type
         FROM friend_share_permissions fsp
         JOIN friend_shares fs ON fs.id = fsp.share_id
         WHERE fs.owner_id = $1 AND fs.friend_id = $2 AND fs.status = 'accepted'",
    )
    .bind(owner_id)
    .bind(friend_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.data_type).collect())
}

/// Get a single share by ID.
pub async fn get_share(pool: &PgPool, share_id: Uuid) -> Result<FriendShareRow, sqlx::Error> {
    sqlx::query_as::<_, FriendShareRow>(
        "SELECT id, owner_id, friend_id, status, invite_token,
                invite_expires_at, created_at, accepted_at, revoked_at
         FROM friend_shares WHERE id = $1",
    )
    .bind(share_id)
    .fetch_one(pool)
    .await
}

/// Helper: fetch permissions for a set of share IDs and build FriendShareResponse list.
async fn build_responses(
    pool: &PgPool,
    shares: Vec<ShareWithUsername>,
    is_outgoing: bool,
    user_id: Uuid,
) -> Result<Vec<FriendShareResponse>, sqlx::Error> {
    if shares.is_empty() {
        return Ok(vec![]);
    }

    let share_ids: Vec<Uuid> = shares.iter().map(|s| s.id).collect();

    let perms = sqlx::query_as::<_, PermissionRow>(
        "SELECT share_id, data_type FROM friend_share_permissions WHERE share_id = ANY($1)",
    )
    .bind(&share_ids)
    .fetch_all(pool)
    .await?;

    // Group permissions by share_id
    let mut perm_map: std::collections::HashMap<Uuid, Vec<String>> =
        std::collections::HashMap::new();
    for p in perms {
        perm_map.entry(p.share_id).or_default().push(p.data_type);
    }

    // We need to look up the current user's username for the "own" side.
    // For outgoing: owner_username = current user, friend_username = peer_username
    // For incoming: owner_username = peer_username, friend_username = current user
    let user_row = sqlx::query_as::<_, (String,)>("SELECT username FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    let my_username = user_row.0;

    let responses = shares
        .into_iter()
        .map(|s| {
            let data_types = perm_map.get(&s.id).cloned().unwrap_or_default();
            if is_outgoing {
                FriendShareResponse {
                    id: s.id,
                    owner_id: s.owner_id,
                    owner_username: my_username.clone(),
                    friend_id: s.friend_id,
                    friend_username: s.peer_username,
                    status: s.status,
                    invite_token: s.invite_token,
                    data_types,
                    created_at: s.created_at,
                    accepted_at: s.accepted_at,
                }
            } else {
                FriendShareResponse {
                    id: s.id,
                    owner_id: s.owner_id,
                    owner_username: s.peer_username.unwrap_or_default(),
                    friend_id: s.friend_id,
                    friend_username: Some(my_username.clone()),
                    status: s.status,
                    invite_token: s.invite_token,
                    data_types,
                    created_at: s.created_at,
                    accepted_at: s.accepted_at,
                }
            }
        })
        .collect();

    Ok(responses)
}
