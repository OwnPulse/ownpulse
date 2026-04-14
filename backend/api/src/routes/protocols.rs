// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::response::IntoResponse;
use chrono::Utc;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db::protocols as db;
use crate::error::ApiError;
use crate::models::protocol::{
    ActiveSubstanceItem, CopyTemplateRequest, CreateProtocol, CreateRunRequest, LogDoseRequest,
    NotificationPreferencesRow, ProtocolDoseRow, ProtocolExport, ProtocolListItem,
    ProtocolResponse, PushTokenRow, RegisterPushTokenRequest, RunResponse, ShareResponse,
    SkipDoseRequest, TemplateListItem, TodaysDoseItem, UpdateNotificationPreferences,
    UpdateProtocol, UpdateRunRequest,
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
        if line.substance.trim().is_empty() {
            return Err(ApiError::BadRequest(
                "substance must not be empty".to_string(),
            ));
        }
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
        && !["active", "paused", "completed", "archived", "draft"].contains(&status.as_str())
    {
        return Err(ApiError::BadRequest(format!(
            "invalid status: {status}. Valid: active, paused, completed, archived, draft"
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

/// POST /protocols/:id/doses/log (legacy — backward compat)
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

/// POST /protocols/:id/doses/skip (legacy — backward compat)
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

// --- Run endpoints ---

/// POST /protocols/:id/runs
pub async fn create_run(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(protocol_id): Path<Uuid>,
    Json(body): Json<CreateRunRequest>,
) -> Result<(StatusCode, Json<RunResponse>), ApiError> {
    let run = db::create_run(&state.pool, protocol_id, user_id, &body).await?;

    // Fetch protocol info for the response
    let duration: Option<i32> =
        sqlx::query_scalar("SELECT duration_days FROM protocols WHERE id = $1")
            .bind(protocol_id)
            .fetch_optional(&state.pool)
            .await?;

    let protocol_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM protocols WHERE id = $1")
            .bind(protocol_id)
            .fetch_optional(&state.pool)
            .await?;

    let today = Utc::now().date_naive();
    let progress_pct = if let Some(dur) = duration {
        if today < run.start_date {
            0.0
        } else {
            let elapsed = (today - run.start_date).num_days() as f64;
            (elapsed / dur as f64 * 100.0).min(100.0)
        }
    } else {
        0.0
    };

    let response = RunResponse {
        id: run.id,
        protocol_id: run.protocol_id,
        protocol_name,
        user_id: run.user_id,
        start_date: run.start_date,
        duration_days: duration,
        status: run.status,
        notify: run.notify,
        notify_time: run.notify_time,
        notify_times: run.notify_times,
        repeat_reminders: run.repeat_reminders,
        repeat_interval_minutes: run.repeat_interval_minutes,
        progress_pct,
        doses_today: 0,
        doses_completed_today: 0,
        created_at: run.created_at,
    };

    publish_event(&state.event_tx, user_id, "protocols", None);
    Ok((StatusCode::CREATED, Json(response)))
}

/// GET /protocols/:id/runs
pub async fn list_runs(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(protocol_id): Path<Uuid>,
) -> Result<Json<Vec<RunResponse>>, ApiError> {
    let runs = db::list_runs(&state.pool, protocol_id, user_id).await?;
    Ok(Json(runs))
}

/// GET /protocols/runs/active
pub async fn list_active_runs(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<Vec<RunResponse>>, ApiError> {
    let runs = db::list_active_runs(&state.pool, user_id).await?;
    Ok(Json(runs))
}

/// PATCH /protocols/runs/:run_id
pub async fn update_run(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(run_id): Path<Uuid>,
    Json(body): Json<UpdateRunRequest>,
) -> Result<StatusCode, ApiError> {
    if let Some(ref status) = body.status
        && !["active", "paused", "completed", "archived"].contains(&status.as_str())
    {
        return Err(ApiError::BadRequest(format!(
            "invalid run status: {status}. Valid: active, paused, completed, archived"
        )));
    }

    let updated = db::update_run(&state.pool, run_id, user_id, &body).await?;
    if !updated {
        return Err(ApiError::NotFound);
    }
    publish_event(&state.event_tx, user_id, "protocols", None);
    Ok(StatusCode::NO_CONTENT)
}

/// POST /protocols/runs/:run_id/doses/log
pub async fn log_dose_on_run(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(run_id): Path<Uuid>,
    Json(body): Json<LogDoseRequest>,
) -> Result<Json<ProtocolDoseRow>, ApiError> {
    let dose = db::log_dose_on_run(&state.pool, user_id, run_id, &body, &state.config).await?;
    publish_event(&state.event_tx, user_id, "protocols", None);
    publish_event(&state.event_tx, user_id, "interventions", None);
    Ok(Json(dose))
}

/// POST /protocols/runs/:run_id/doses/skip
pub async fn skip_dose_on_run(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(run_id): Path<Uuid>,
    Json(body): Json<SkipDoseRequest>,
) -> Result<StatusCode, ApiError> {
    db::skip_dose_on_run(&state.pool, user_id, run_id, &body).await?;
    publish_event(&state.event_tx, user_id, "protocols", None);
    Ok(StatusCode::NO_CONTENT)
}

// --- Existing endpoints ---

/// POST /protocols/:id/share
pub async fn share_protocol(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ShareResponse>, ApiError> {
    let (token, expires_at) = db::generate_share_token(&state.pool, id, user_id).await?;
    Ok(Json(ShareResponse { token, expires_at }))
}

/// GET /protocols/shared/:token -- no auth required
pub async fn get_shared_protocol(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<ProtocolResponse>, ApiError> {
    let mut response = db::get_shared(&state.pool, &token).await?;
    // Strip private fields from public response
    response.user_id = None;
    response.share_token = None;
    response.share_expires_at = None;
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

/// GET /protocols/runs/todays-doses
pub async fn todays_doses(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<Vec<TodaysDoseItem>>, ApiError> {
    let rows = db::todays_doses(&state.pool, user_id).await?;
    Ok(Json(rows))
}

/// GET /protocols/active-substances
pub async fn active_substances(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<Vec<ActiveSubstanceItem>>, ApiError> {
    let rows = db::active_substances(&state.pool, user_id).await?;
    Ok(Json(rows))
}

/// GET /protocols/templates
pub async fn list_templates(
    State(state): State<AppState>,
    AuthUser { .. }: AuthUser,
) -> Result<Json<Vec<TemplateListItem>>, ApiError> {
    let rows = db::list_templates(&state.pool).await?;
    Ok(Json(rows))
}

/// GET /protocols/:id/export
pub async fn export_protocol(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let export = db::export_protocol(&state.pool, id, user_id).await?;
    let json =
        serde_json::to_string_pretty(&export).map_err(|e| ApiError::Internal(e.to_string()))?;
    let filename = format!("{}.json", export.name.replace(' ', "_"));

    Ok((
        [
            (CONTENT_TYPE, "application/json".to_string()),
            (
                CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        json,
    ))
}

/// POST /protocols/import
pub async fn import_protocol_file(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<ProtocolExport>,
) -> Result<(StatusCode, Json<ProtocolResponse>), ApiError> {
    // Validate
    if body.schema != "ownpulse-protocol/v1" {
        return Err(ApiError::BadRequest(
            "unsupported schema; expected ownpulse-protocol/v1".to_string(),
        ));
    }
    if body.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name must not be empty".to_string()));
    }
    if body.duration_days < 1 || body.duration_days > 365 {
        return Err(ApiError::BadRequest(
            "duration_days must be between 1 and 365".to_string(),
        ));
    }
    if body.lines.is_empty() {
        return Err(ApiError::BadRequest(
            "at least one line is required".to_string(),
        ));
    }

    let today = Utc::now().date_naive();
    let protocol = db::import_protocol_from_export(&state.pool, user_id, today, &body).await?;
    let response = db::get_by_id(&state.pool, protocol.id, user_id).await?;
    publish_event(&state.event_tx, user_id, "protocols", None);
    Ok((StatusCode::CREATED, Json(response)))
}

/// POST /protocols/templates/:id/copy
pub async fn copy_template(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CopyTemplateRequest>,
) -> Result<(StatusCode, Json<ProtocolResponse>), ApiError> {
    let protocol = db::copy_template(&state.pool, id, user_id, body.start_date).await?;
    let response = db::get_by_id(&state.pool, protocol.id, user_id).await?;
    publish_event(&state.event_tx, user_id, "protocols", None);
    Ok((StatusCode::CREATED, Json(response)))
}

// --- Notification preferences ---

/// GET /protocols/notifications
pub async fn get_notification_preferences(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<NotificationPreferencesRow>, ApiError> {
    let prefs = db::get_notification_preferences(&state.pool, user_id).await?;
    Ok(Json(prefs))
}

/// PUT /protocols/notifications
pub async fn update_notification_preferences(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<UpdateNotificationPreferences>,
) -> Result<Json<NotificationPreferencesRow>, ApiError> {
    let prefs = db::update_notification_preferences(&state.pool, user_id, &body).await?;
    Ok(Json(prefs))
}

// --- Push tokens ---

/// POST /notifications/push-token
pub async fn register_push_token(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<RegisterPushTokenRequest>,
) -> Result<(StatusCode, Json<PushTokenRow>), ApiError> {
    if body.device_token.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "device_token must not be empty".to_string(),
        ));
    }
    if !["ios", "web"].contains(&body.platform.as_str()) {
        return Err(ApiError::BadRequest(
            "platform must be 'ios' or 'web'".to_string(),
        ));
    }

    let token = db::register_push_token(&state.pool, user_id, &body).await?;
    Ok((StatusCode::CREATED, Json(token)))
}

/// DELETE /notifications/push-token/:device_token
pub async fn delete_push_token(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(device_token): Path<String>,
) -> Result<StatusCode, ApiError> {
    let deleted = db::delete_push_token(&state.pool, user_id, &device_token).await?;
    if !deleted {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
