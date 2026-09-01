// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db::interventions as db;
use crate::error::ApiError;
use crate::models::intervention::{
    CreateIntervention, InterventionQuery, InterventionRow, UpdateIntervention,
};
use crate::routes::events::publish_event;

/// POST /interventions — no substance name validation per project rules.
///
/// Idempotent for synced records: when `source_id` is set and a row with
/// the same (user, source, source_id) already exists, returns 200 with the
/// existing row instead of creating a duplicate. Fresh inserts return 201.
pub async fn create(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<CreateIntervention>,
) -> Result<(StatusCode, Json<InterventionRow>), ApiError> {
    // source_id is btree-indexed; oversized values would fail the insert
    // with a 500 instead of a clean rejection.
    for (field, value) in [("source", &body.source), ("source_id", &body.source_id)] {
        if let Some(value) = value
            && value.len() > 255
        {
            return Err(ApiError::BadRequest(format!(
                "{field} must be at most 255 characters"
            )));
        }
    }

    if let Some(row) = db::insert(&state.pool, user_id, &body).await? {
        publish_event(&state.event_tx, user_id, "interventions", None);
        return Ok((StatusCode::CREATED, Json(row)));
    }

    // The insert conflicted, which only happens with a source_id present.
    let source = body.source.as_deref().unwrap_or("manual");
    let Some(source_id) = body.source_id.as_deref() else {
        return Err(ApiError::Internal(
            "intervention insert returned no row without a source_id".to_string(),
        ));
    };

    if let Some(existing) = db::get_by_source_id(&state.pool, user_id, source, source_id).await? {
        // source_id is client-controlled free text, so keep it out of the
        // logs; the row id is enough to look it up.
        tracing::info!(source, intervention_id = %existing.id,
            "replayed intervention create; returning existing row");
        return Ok((StatusCode::OK, Json(existing)));
    }

    // The conflicting row was deleted between insert and fetch. Retry the
    // insert, then the fetch — a second conflict means a row with this
    // identity exists again, and returning it keeps the endpoint 2xx for
    // sync clients, which treat any other status as a failed upload.
    if let Some(row) = db::insert(&state.pool, user_id, &body).await? {
        publish_event(&state.event_tx, user_id, "interventions", None);
        return Ok((StatusCode::CREATED, Json(row)));
    }
    let existing = db::get_by_source_id(&state.pool, user_id, source, source_id)
        .await?
        .ok_or_else(|| {
            ApiError::Conflict(
                "intervention create raced with concurrent deletes; retry".to_string(),
            )
        })?;
    tracing::info!(source, intervention_id = %existing.id,
        "replayed intervention create; returning existing row");
    Ok((StatusCode::OK, Json(existing)))
}

/// GET /interventions
pub async fn list(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Query(query): Query<InterventionQuery>,
) -> Result<Json<Vec<InterventionRow>>, ApiError> {
    let rows = db::list(&state.pool, user_id, query.start, query.end).await?;
    Ok(Json(rows))
}

/// GET /interventions/:id
pub async fn get(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<InterventionRow>, ApiError> {
    let row = db::get_by_id(&state.pool, user_id, id).await?;
    Ok(Json(row))
}

/// PATCH /interventions/:id — all fields optional; unset fields are left
/// unchanged. No substance-name validation per project rules.
pub async fn update(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateIntervention>,
) -> Result<Json<InterventionRow>, ApiError> {
    if let Some(ref substance) = body.substance
        && substance.trim().is_empty()
    {
        return Err(ApiError::BadRequest(
            "substance must not be empty".to_string(),
        ));
    }

    let row = db::update(&state.pool, user_id, id, &body)
        .await?
        .ok_or(ApiError::NotFound)?;
    publish_event(&state.event_tx, user_id, "interventions", None);
    Ok(Json(row))
}

/// DELETE /interventions/:id
pub async fn delete(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    db::delete(&state.pool, user_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
