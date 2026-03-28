// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::observer_poll::{
    ExportRow, MemberRow, ObserverPollRow, ObserverResponseView, OwnerResponseView, PollRow,
    ResponseRow,
};

pub async fn create_poll(
    pool: &PgPool,
    user_id: Uuid,
    name: &str,
    custom_prompt: Option<&str>,
    dimensions: &serde_json::Value,
) -> Result<PollRow, sqlx::Error> {
    sqlx::query_as::<_, PollRow>(
        "INSERT INTO observer_polls (user_id, name, custom_prompt, dimensions)
         VALUES ($1, $2, $3, $4)
         RETURNING id, user_id, name, custom_prompt, dimensions, created_at, deleted_at",
    )
    .bind(user_id)
    .bind(name)
    .bind(custom_prompt)
    .bind(dimensions)
    .fetch_one(pool)
    .await
}

pub async fn list_polls(pool: &PgPool, user_id: Uuid) -> Result<Vec<PollRow>, sqlx::Error> {
    sqlx::query_as::<_, PollRow>(
        "SELECT id, user_id, name, custom_prompt, dimensions, created_at, deleted_at
         FROM observer_polls
         WHERE user_id = $1 AND deleted_at IS NULL
         ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn get_poll(
    pool: &PgPool,
    poll_id: Uuid,
    user_id: Uuid,
) -> Result<Option<PollRow>, sqlx::Error> {
    sqlx::query_as::<_, PollRow>(
        "SELECT id, user_id, name, custom_prompt, dimensions, created_at, deleted_at
         FROM observer_polls
         WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(poll_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

pub async fn update_poll(
    pool: &PgPool,
    poll_id: Uuid,
    user_id: Uuid,
    name: Option<&str>,
    custom_prompt: Option<&str>,
) -> Result<Option<PollRow>, sqlx::Error> {
    sqlx::query_as::<_, PollRow>(
        "UPDATE observer_polls
         SET name = COALESCE($3, name),
             custom_prompt = COALESCE($4, custom_prompt)
         WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL
         RETURNING id, user_id, name, custom_prompt, dimensions, created_at, deleted_at",
    )
    .bind(poll_id)
    .bind(user_id)
    .bind(name)
    .bind(custom_prompt)
    .fetch_optional(pool)
    .await
}

pub async fn soft_delete_poll(
    pool: &PgPool,
    poll_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE observer_polls
         SET deleted_at = now()
         WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(poll_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn create_invite(
    pool: &PgPool,
    poll_id: Uuid,
) -> Result<(Uuid, DateTime<Utc>), sqlx::Error> {
    let row: (Uuid, DateTime<Utc>) = sqlx::query_as(
        "INSERT INTO observer_poll_members (poll_id)
         VALUES ($1)
         RETURNING invite_token, invite_expires_at",
    )
    .bind(poll_id)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn accept_invite(
    pool: &PgPool,
    invite_token: Uuid,
    observer_id: Uuid,
) -> Result<Option<PollRow>, sqlx::Error> {
    // Use a transaction to update the member and fetch the poll atomically.
    let mut tx = pool.begin().await?;

    let updated = sqlx::query(
        "UPDATE observer_poll_members
         SET observer_id = $2, accepted_at = now()
         WHERE invite_token = $1
           AND accepted_at IS NULL
           AND invite_expires_at > now()",
    )
    .bind(invite_token)
    .bind(observer_id)
    .execute(&mut *tx)
    .await?;

    if updated.rows_affected() == 0 {
        tx.commit().await?;
        return Ok(None);
    }

    let poll = sqlx::query_as::<_, PollRow>(
        "SELECT p.id, p.user_id, p.name, p.custom_prompt, p.dimensions, p.created_at, p.deleted_at
         FROM observer_polls p
         JOIN observer_poll_members m ON m.poll_id = p.id
         WHERE m.invite_token = $1",
    )
    .bind(invite_token)
    .fetch_optional(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(poll)
}

pub async fn list_members(pool: &PgPool, poll_id: Uuid) -> Result<Vec<MemberRow>, sqlx::Error> {
    sqlx::query_as::<_, MemberRow>(
        "SELECT m.id, u.email AS observer_email, m.accepted_at, m.created_at
         FROM observer_poll_members m
         LEFT JOIN users u ON u.id = m.observer_id
         WHERE m.poll_id = $1
         ORDER BY m.created_at DESC",
    )
    .bind(poll_id)
    .fetch_all(pool)
    .await
}

pub async fn list_observer_polls(
    pool: &PgPool,
    observer_id: Uuid,
) -> Result<Vec<ObserverPollRow>, sqlx::Error> {
    sqlx::query_as::<_, ObserverPollRow>(
        "SELECT p.id, u.email AS owner_email, p.name, p.custom_prompt, p.dimensions
         FROM observer_polls p
         JOIN observer_poll_members m ON m.poll_id = p.id
         JOIN users u ON u.id = p.user_id
         WHERE m.observer_id = $1
           AND m.accepted_at IS NOT NULL
           AND p.deleted_at IS NULL
         ORDER BY p.created_at DESC",
    )
    .bind(observer_id)
    .fetch_all(pool)
    .await
}

/// Returns the member_id for the given observer on the given poll, if accepted.
pub async fn get_accepted_member_id(
    pool: &PgPool,
    poll_id: Uuid,
    observer_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM observer_poll_members
         WHERE poll_id = $1 AND observer_id = $2 AND accepted_at IS NOT NULL",
    )
    .bind(poll_id)
    .bind(observer_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.0))
}

pub async fn upsert_response(
    pool: &PgPool,
    poll_id: Uuid,
    member_id: Uuid,
    date: NaiveDate,
    scores: &serde_json::Value,
) -> Result<(ResponseRow, bool), sqlx::Error> {
    // Check if a response already exists for this member+date.
    let existed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM observer_responses WHERE member_id = $1 AND date = $2)",
    )
    .bind(member_id)
    .bind(date)
    .fetch_one(pool)
    .await?;

    let row = sqlx::query_as::<_, ResponseRow>(
        "INSERT INTO observer_responses (poll_id, member_id, date, scores)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (member_id, date)
         DO UPDATE SET scores = EXCLUDED.scores
         RETURNING id, poll_id, member_id, date, scores, created_at",
    )
    .bind(poll_id)
    .bind(member_id)
    .bind(date)
    .bind(scores)
    .fetch_one(pool)
    .await?;

    Ok((row, !existed))
}

pub async fn list_responses_for_owner(
    pool: &PgPool,
    poll_id: Uuid,
    user_id: Uuid,
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
) -> Result<Vec<OwnerResponseView>, sqlx::Error> {
    sqlx::query_as::<_, OwnerResponseView>(
        "SELECT r.id, r.member_id, u.email AS observer_email, r.date, r.scores, r.created_at
         FROM observer_responses r
         JOIN observer_poll_members m ON m.id = r.member_id
         JOIN observer_polls p ON p.id = r.poll_id
         LEFT JOIN users u ON u.id = m.observer_id
         WHERE r.poll_id = $1
           AND p.user_id = $2
           AND ($3::DATE IS NULL OR r.date >= $3)
           AND ($4::DATE IS NULL OR r.date <= $4)
         ORDER BY r.date DESC, r.created_at DESC",
    )
    .bind(poll_id)
    .bind(user_id)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
}

pub async fn list_my_responses(
    pool: &PgPool,
    member_id: Uuid,
) -> Result<Vec<ObserverResponseView>, sqlx::Error> {
    sqlx::query_as::<_, ObserverResponseView>(
        "SELECT id, date, scores, created_at
         FROM observer_responses
         WHERE member_id = $1
         ORDER BY date DESC",
    )
    .bind(member_id)
    .fetch_all(pool)
    .await
}

pub async fn delete_response(
    pool: &PgPool,
    response_id: Uuid,
    observer_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM observer_responses
         WHERE id = $1
           AND member_id IN (
               SELECT id FROM observer_poll_members WHERE observer_id = $2
           )",
    )
    .bind(response_id)
    .bind(observer_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn export_observer_responses(
    pool: &PgPool,
    observer_id: Uuid,
) -> Result<Vec<ExportRow>, sqlx::Error> {
    sqlx::query_as::<_, ExportRow>(
        "SELECT r.id, p.name AS poll_name, r.date, r.scores, r.created_at
         FROM observer_responses r
         JOIN observer_poll_members m ON m.id = r.member_id
         JOIN observer_polls p ON p.id = r.poll_id
         WHERE m.observer_id = $1
         ORDER BY r.date DESC, r.created_at DESC",
    )
    .bind(observer_id)
    .fetch_all(pool)
    .await
}

/// Check that the poll exists and is not deleted (used for invite creation).
pub async fn poll_exists_for_user(
    pool: &PgPool,
    poll_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM observer_polls
            WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL
        )",
    )
    .bind(poll_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(exists)
}

/// Fetch dimensions for a poll by ID.
pub async fn get_poll_dimensions(
    pool: &PgPool,
    poll_id: Uuid,
) -> Result<Option<serde_json::Value>, sqlx::Error> {
    let row: Option<(serde_json::Value,)> = sqlx::query_as(
        "SELECT dimensions FROM observer_polls WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(poll_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.0))
}
