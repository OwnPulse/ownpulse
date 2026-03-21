// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{NaiveTime, TimeZone, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::auth::extractor::AuthUser;
use crate::db::observations as db;
use crate::error::ApiError;
use crate::models::observation::CreateObservation;
use crate::models::sleep::{CreateSleep, SleepQuery, SleepResponse};
use crate::AppState;

const SLEEP_TYPE: &str = "sleep";

/// POST /sleep — create a sleep observation.
pub async fn create(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<CreateSleep>,
) -> Result<(StatusCode, Json<SleepResponse>), ApiError> {
    let source = body.source.as_deref().unwrap_or("manual");

    // Compute start_time and end_time from the request.
    let start_time = body.sleep_start.unwrap_or_else(|| {
        let midnight = body
            .date
            .and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        Utc.from_utc_datetime(&midnight)
    });
    let end_time = body.sleep_end.or_else(|| {
        Some(start_time + chrono::Duration::minutes(i64::from(body.duration_minutes)))
    });

    // Pack all sleep-specific fields into the JSONB value.
    let value = json!({
        "duration_minutes": body.duration_minutes,
        "deep_minutes": body.deep_minutes,
        "light_minutes": body.light_minutes,
        "rem_minutes": body.rem_minutes,
        "awake_minutes": body.awake_minutes,
        "score": body.score,
        "source_id": body.source_id,
        "notes": body.notes,
    });

    let obs = CreateObservation {
        obs_type: SLEEP_TYPE.to_string(),
        name: source.to_string(),
        start_time,
        end_time,
        value: Some(value),
        source: Some(source.to_string()),
        metadata: None,
    };

    let row = db::insert(&state.pool, user_id, &obs).await?;
    Ok((StatusCode::CREATED, Json(SleepResponse::from_observation(row))))
}

/// GET /sleep — list sleep observations with optional date range.
pub async fn list(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Query(query): Query<SleepQuery>,
) -> Result<Json<Vec<SleepResponse>>, ApiError> {
    let rows = db::list_by_type_with_date_range(
        &state.pool,
        user_id,
        SLEEP_TYPE,
        query.start,
        query.end,
    )
    .await?;

    let records: Vec<SleepResponse> = rows
        .into_iter()
        .map(SleepResponse::from_observation)
        .collect();
    Ok(Json(records))
}

/// GET /sleep/:id
pub async fn get(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<SleepResponse>, ApiError> {
    let row = db::get_by_id(&state.pool, user_id, id).await?;

    if row.obs_type != SLEEP_TYPE {
        return Err(ApiError::NotFound);
    }

    Ok(Json(SleepResponse::from_observation(row)))
}

/// DELETE /sleep/:id
pub async fn delete(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = db::delete(&state.pool, user_id, id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}
