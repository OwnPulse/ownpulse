// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db::checkins as db;
use crate::error::ApiError;
use crate::models::checkin::{CheckinQuery, CheckinRow, UpsertCheckin};

fn validate_score(value: Option<i32>, field: &str) -> Result<(), ApiError> {
    if let Some(v) = value
        && !(1..=10).contains(&v)
    {
        return Err(ApiError::BadRequest(format!(
            "{field} must be between 1 and 10"
        )));
    }
    Ok(())
}

/// POST /checkins — upsert by date; validates all scores are 1-10 if provided.
pub async fn upsert(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<UpsertCheckin>,
) -> Result<(StatusCode, Json<CheckinRow>), ApiError> {
    validate_score(body.energy, "energy")?;
    validate_score(body.mood, "mood")?;
    validate_score(body.focus, "focus")?;
    validate_score(body.recovery, "recovery")?;
    validate_score(body.libido, "libido")?;

    let row = db::upsert(&state.pool, user_id, &body).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

/// GET /checkins
pub async fn list(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Query(_query): Query<CheckinQuery>,
) -> Result<Json<Vec<CheckinRow>>, ApiError> {
    let rows = db::list(&state.pool, user_id).await?;
    Ok(Json(rows))
}

/// GET /checkins/:id
pub async fn get(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<CheckinRow>, ApiError> {
    let row = db::get_by_id(&state.pool, user_id, id).await?;
    Ok(Json(row))
}

/// DELETE /checkins/:id
pub async fn delete(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    db::delete(&state.pool, user_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
