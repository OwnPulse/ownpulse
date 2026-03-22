// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::error::ApiError;

#[derive(Serialize)]
pub struct DashboardSummary {
    pub latest_checkin: Option<LatestCheckin>,
    pub checkin_count_7d: i64,
    pub health_record_count_7d: i64,
    pub intervention_count_7d: i64,
    pub observation_count_7d: i64,
    pub latest_lab_date: Option<chrono::NaiveDate>,
    pub pending_friend_shares: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct LatestCheckin {
    pub date: chrono::NaiveDate,
    pub energy: Option<i32>,
    pub mood: Option<i32>,
    pub focus: Option<i32>,
    pub recovery: Option<i32>,
    pub libido: Option<i32>,
}

/// GET /dashboard/summary
pub async fn summary(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<DashboardSummary>, ApiError> {
    let latest_checkin = sqlx::query_as::<_, LatestCheckin>(
        "SELECT date, energy, mood, focus, recovery, libido \
         FROM daily_checkins WHERE user_id = $1 ORDER BY date DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;

    let (checkin_count_7d,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM daily_checkins \
         WHERE user_id = $1 AND date >= CURRENT_DATE - INTERVAL '7 days'",
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?;

    let (health_record_count_7d,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM health_records \
         WHERE user_id = $1 AND start_time >= now() - INTERVAL '7 days'",
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?;

    let (intervention_count_7d,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM interventions \
         WHERE user_id = $1 AND administered_at >= now() - INTERVAL '7 days'",
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?;

    let (observation_count_7d,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM observations \
         WHERE user_id = $1 AND start_time >= now() - INTERVAL '7 days'",
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?;

    let latest_lab_date: Option<(chrono::NaiveDate,)> = sqlx::query_as(
        "SELECT panel_date FROM lab_results \
         WHERE user_id = $1 ORDER BY panel_date DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;

    let (pending_friend_shares,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM friend_shares \
         WHERE friend_id = $1 AND status = 'pending'",
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(DashboardSummary {
        latest_checkin,
        checkin_count_7d,
        health_record_count_7d,
        intervention_count_7d,
        observation_count_7d,
        latest_lab_date: latest_lab_date.map(|(d,)| d),
        pending_friend_shares,
    }))
}
