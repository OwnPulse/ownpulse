// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db::{explore as db_explore, explore_charts as db_charts};
use crate::error::ApiError;
use crate::models::explore::{
    BatchSeriesRequest, CalendarField, CheckinField, ChartRow, CreateChart, HealthRecordField,
    MetricOption, MetricSource, MetricSourceGroup, MetricsResponse, SeriesQuery, SeriesResponse,
    SleepField, UpdateChart, validate_chart_config,
};

/// GET /explore/metrics — list available metrics for the user.
pub async fn metrics(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<MetricsResponse>, ApiError> {
    let lab_markers = db_explore::distinct_lab_markers(&state.pool, user_id).await?;

    let sources = vec![
        MetricSourceGroup {
            source: "health_records".to_string(),
            label: "Health Records".to_string(),
            metrics: HealthRecordField::all()
                .iter()
                .map(|f| MetricOption {
                    field: f.record_type().to_string(),
                    label: f.label().to_string(),
                    unit: f.unit().to_string(),
                })
                .collect(),
        },
        MetricSourceGroup {
            source: "checkins".to_string(),
            label: "Check-ins".to_string(),
            metrics: CheckinField::all()
                .iter()
                .map(|f| MetricOption {
                    field: f.column().to_string(),
                    label: f.label().to_string(),
                    unit: "score".to_string(),
                })
                .collect(),
        },
        MetricSourceGroup {
            source: "labs".to_string(),
            label: "Lab Results".to_string(),
            metrics: lab_markers
                .into_iter()
                .map(|m| MetricOption {
                    field: m.clone(),
                    label: m,
                    unit: "value".to_string(),
                })
                .collect(),
        },
        MetricSourceGroup {
            source: "calendar".to_string(),
            label: "Calendar".to_string(),
            metrics: CalendarField::all()
                .iter()
                .map(|f| MetricOption {
                    field: f.column().to_string(),
                    label: f.label().to_string(),
                    unit: f.unit().to_string(),
                })
                .collect(),
        },
        MetricSourceGroup {
            source: "sleep".to_string(),
            label: "Sleep".to_string(),
            metrics: SleepField::all()
                .iter()
                .map(|f| MetricOption {
                    field: f.json_key().to_string(),
                    label: f.label().to_string(),
                    unit: f.unit().to_string(),
                })
                .collect(),
        },
    ];

    Ok(Json(MetricsResponse { sources }))
}

/// GET /explore/series — single time-series with aggregation.
pub async fn series_get(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Query(query): Query<SeriesQuery>,
) -> Result<Json<SeriesResponse>, ApiError> {
    let metric = MetricSource::parse(&query.source, &query.field)?;
    let result =
        db_explore::query_series(&state.pool, user_id, &metric, query.start, query.end, query.resolution)
            .await?;
    Ok(Json(result))
}

/// POST /explore/series — batch time-series (multiple metrics).
pub async fn series_post(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<BatchSeriesRequest>,
) -> Result<Json<Vec<SeriesResponse>>, ApiError> {
    if body.metrics.is_empty() {
        return Err(ApiError::BadRequest(
            "at least one metric is required".to_string(),
        ));
    }
    if body.metrics.len() > 8 {
        return Err(ApiError::BadRequest(
            "at most 8 metrics per request".to_string(),
        ));
    }

    // Validate all metrics before executing any queries.
    let parsed: Vec<MetricSource> = body
        .metrics
        .iter()
        .map(|m| MetricSource::parse(&m.source, &m.field))
        .collect::<Result<Vec<_>, _>>()?;

    // Execute queries in parallel.
    let futures: Vec<_> = parsed
        .iter()
        .map(|metric| {
            db_explore::query_series(&state.pool, user_id, metric, body.start, body.end, body.resolution)
        })
        .collect();

    let results = futures::future::join_all(futures).await;
    let series: Vec<SeriesResponse> = results.into_iter().collect::<Result<Vec<_>, _>>()?;

    Ok(Json(series))
}

/// POST /explore/charts — create a saved chart.
pub async fn create_chart(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<CreateChart>,
) -> Result<(StatusCode, Json<ChartRow>), ApiError> {
    validate_chart_config(&body.config)?;

    let config_json = serde_json::to_value(&body.config)
        .map_err(|e| ApiError::Internal(format!("failed to serialize chart config: {e}")))?;

    let row = db_charts::insert(&state.pool, user_id, &body.name, &config_json).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

/// GET /explore/charts — list saved charts.
pub async fn list_charts(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
) -> Result<Json<Vec<ChartRow>>, ApiError> {
    let rows = db_charts::list(&state.pool, user_id).await?;
    Ok(Json(rows))
}

/// GET /explore/charts/:id
pub async fn get_chart(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ChartRow>, ApiError> {
    let row = db_charts::get_by_id(&state.pool, user_id, id).await?;
    Ok(Json(row))
}

/// PUT /explore/charts/:id
pub async fn update_chart(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateChart>,
) -> Result<Json<ChartRow>, ApiError> {
    let config_json = match &body.config {
        Some(config) => {
            validate_chart_config(config)?;
            Some(
                serde_json::to_value(config).map_err(|e| {
                    ApiError::Internal(format!("failed to serialize chart config: {e}"))
                })?,
            )
        }
        None => None,
    };

    let row = db_charts::update(
        &state.pool,
        user_id,
        id,
        body.name.as_deref(),
        config_json.as_ref(),
    )
    .await?;

    Ok(Json(row))
}

/// DELETE /explore/charts/:id
pub async fn delete_chart(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = db_charts::delete(&state.pool, user_id, id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}
