// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db::source_preferences as db;
use crate::error::ApiError;
use crate::models::source_preference::{
    KNOWN_HEALTH_RECORD_SOURCES, SourcePreferenceRow, UpsertSourcePreference,
};

/// GET /source-preferences
pub async fn list(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<Vec<SourcePreferenceRow>>, ApiError> {
    let rows = db::list(&state.pool, user_id).await?;
    Ok(Json(rows))
}

/// POST /source-preferences — upsert a per-metric source preference.
pub async fn upsert(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<UpsertSourcePreference>,
) -> Result<(StatusCode, Json<SourcePreferenceRow>), ApiError> {
    // A preference naming a source that can never appear on a health_records
    // row can never match anything in SOURCE_PREFERENCE_EXCLUSION's
    // dedup-partner walk — it would be silently inert rather than doing
    // anything wrong, but inert-and-invisible is still a footgun (e.g. a
    // typo'd "garmn" quietly never applies). Reject it outright instead.
    if !KNOWN_HEALTH_RECORD_SOURCES.contains(&body.preferred_source.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "unknown preferred_source '{}'; must be one of: {}",
            body.preferred_source,
            KNOWN_HEALTH_RECORD_SOURCES.join(", ")
        )));
    }

    let row = db::upsert(
        &state.pool,
        user_id,
        &body.metric_type,
        &body.preferred_source,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(row)))
}
