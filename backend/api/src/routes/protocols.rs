// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db::protocols as db;
use crate::error::ApiError;
use crate::models::protocol::{
    CreateProtocol, LogDoseRequest, ProtocolDoseRow, ProtocolListItem, ProtocolResponse,
    ShareResponse, SkipDoseRequest, TodaysDoseItem, UpdateProtocol,
};
use crate::routes::events::publish_event;

/// POST /protocols
pub async fn create_protocol(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<CreateProtocol>,
) -> Result<(StatusCode, Json<ProtocolResponse>), ApiError> {
    // Validate
    if body.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name must not be empty".to_string()));
    }
    if body.duration_days < 1 || body.duration_days > 365 {
        return Err(ApiError::BadRequest(
            "duration_days must be between 1 and 365".to_string(),
        ));
    }
    for line in &body.lines {
        if line.schedule_pattern.len() != body.duration_days as usize {
            return Err(ApiError::BadRequest(format!(
                "schedule_pattern length ({}) must equal duration_days ({})",
                line.schedule_pattern.len(),
                body.duration_days
            )));
        }
    }

    let protocol = db::insert(&state.pool, user_id, &body).await?;
    let response = db::get_by_id(&state.pool, protocol.id, user_id).await?;
    publish_event(&state.event_tx, user_id, "protocols", None);
    Ok((StatusCode::CREATED, Json(response)))
}

/// GET /protocols
pub async fn list_protocols(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<Vec<ProtocolListItem>>, ApiError> {
    let rows = db::list(&state.pool, user_id).await?;
    Ok(Json(rows))
}

/// GET /protocols/:id
pub async fn get_protocol(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ProtocolResponse>, ApiError> {
    let response = db::get_by_id(&state.pool, id, user_id).await?;
    Ok(Json(response))
}

/// PATCH /protocols/:id
pub async fn update_protocol(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateProtocol>,
) -> Result<StatusCode, ApiError> {
    if let Some(ref status) = body.status
        && !["active", "paused", "completed", "archived"].contains(&status.as_str())
    {
        return Err(ApiError::BadRequest(format!(
            "invalid status: {status}. Valid: active, paused, completed, archived"
        )));
    }

    let updated = db::update(&state.pool, id, user_id, &body).await?;
    if !updated {
        return Err(ApiError::NotFound);
    }
    publish_event(&state.event_tx, user_id, "protocols", None);
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /protocols/:id
pub async fn delete_protocol(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = db::delete(&state.pool, id, user_id).await?;
    if !deleted {
        return Err(ApiError::NotFound);
    }
    publish_event(&state.event_tx, user_id, "protocols", None);
    Ok(StatusCode::NO_CONTENT)
}

/// POST /protocols/:id/doses/log
pub async fn log_dose(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<LogDoseRequest>,
) -> Result<Json<ProtocolDoseRow>, ApiError> {
    let dose = db::log_dose(&state.pool, user_id, id, &body, &state.config).await?;
    publish_event(&state.event_tx, user_id, "protocols", None);
    publish_event(&state.event_tx, user_id, "interventions", None);
    Ok(Json(dose))
}

/// POST /protocols/:id/doses/skip
pub async fn skip_dose(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SkipDoseRequest>,
) -> Result<StatusCode, ApiError> {
    db::skip_dose(&state.pool, user_id, id, &body).await?;
    publish_event(&state.event_tx, user_id, "protocols", None);
    Ok(StatusCode::NO_CONTENT)
}

/// POST /protocols/:id/share
pub async fn share_protocol(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ShareResponse>, ApiError> {
    let (token, expires_at) = db::generate_share_token(&state.pool, id, user_id).await?;
    Ok(Json(ShareResponse { token, expires_at }))
}

/// GET /protocols/shared/:token — no auth required
pub async fn get_shared_protocol(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<ProtocolResponse>, ApiError> {
    let response = db::get_shared(&state.pool, &token).await?;
    Ok(Json(response))
}

/// POST /protocols/import/:token
pub async fn import_protocol(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(token): Path<String>,
) -> Result<(StatusCode, Json<ProtocolResponse>), ApiError> {
    let protocol = db::import_protocol(&state.pool, user_id, &token).await?;
    let response = db::get_by_id(&state.pool, protocol.id, user_id).await?;
    publish_event(&state.event_tx, user_id, "protocols", None);
    Ok((StatusCode::CREATED, Json(response)))
}

/// GET /protocols/todays-doses
pub async fn todays_doses(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<Vec<TodaysDoseItem>>, ApiError> {
    let rows = db::todays_doses(&state.pool, user_id).await?;
    Ok(Json(rows))
}
