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
use crate::models::checkin::{CheckinInput, CheckinQuery, CheckinRow};
use crate::routes::events::publish_event;

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

/// POST /checkins — create; validates all scores are 1-10 if provided.
pub async fn create(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<CheckinInput>,
) -> Result<(StatusCode, Json<CheckinRow>), ApiError> {
    validate_score(body.energy, "energy")?;
    validate_score(body.mood, "mood")?;
    validate_score(body.focus, "focus")?;
    validate_score(body.recovery, "recovery")?;
    validate_score(body.libido, "libido")?;

    let row = db::create(&state.pool, user_id, &body).await?;
    publish_event(&state.event_tx, user_id, "checkins", None);
    Ok((StatusCode::CREATED, Json(row)))
}

/// PUT /checkins/:id — update an existing check-in.
pub async fn update(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(checkin_id): Path<Uuid>,
    Json(body): Json<CheckinInput>,
) -> Result<Json<CheckinRow>, ApiError> {
    validate_score(body.energy, "energy")?;
    validate_score(body.mood, "mood")?;
    validate_score(body.focus, "focus")?;
    validate_score(body.recovery, "recovery")?;
    validate_score(body.libido, "libido")?;

    let row = db::update(&state.pool, user_id, checkin_id, &body).await?;
    publish_event(&state.event_tx, user_id, "checkins", None);
    Ok(Json(row))
}

/// GET /checkins
pub async fn list(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Query(query): Query<CheckinQuery>,
) -> Result<Json<Vec<CheckinRow>>, ApiError> {
    let rows = db::list(&state.pool, user_id, query.start, query.end).await?;
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
