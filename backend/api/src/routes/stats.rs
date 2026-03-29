// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Route handlers for the correlation explorer endpoints.

use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use chrono::{Duration, Utc};

use crate::AppState;
use crate::auth::extractor::AuthUser;
use crate::db::{explore as db_explore, stats as db_stats};
use crate::error::ApiError;
use crate::models::explore::DataPoint;
use crate::models::stats::{
    BeforeAfterRequest, BeforeAfterResponse, BestLag, CorrelateRequest, CorrelateResponse,
    CorrelationMethod, LagCorrelateRequest, LagCorrelateResponse, LagResult, MetricRefOut,
    ScatterPoint, TimeValue, WindowStats,
};
use crate::stats;

/// POST /stats/before-after
pub async fn before_after(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<BeforeAfterRequest>,
) -> Result<Json<BeforeAfterResponse>, ApiError> {
    // Validation
    let substance = body.intervention_substance.trim();
    if substance.is_empty() {
        return Err(ApiError::BadRequest(
            "intervention_substance must not be empty".to_string(),
        ));
    }
    if body.before_days < 1 || body.before_days > 365 {
        return Err(ApiError::BadRequest(
            "before_days must be between 1 and 365".to_string(),
        ));
    }
    if body.after_days < 1 || body.after_days > 365 {
        return Err(ApiError::BadRequest(
            "after_days must be between 1 and 365".to_string(),
        ));
    }
    let metric = body.metric.parse()?;

    let metric_out = MetricRefOut {
        source: body.metric.source.clone(),
        field: body.metric.field.clone(),
    };

    // Find dose range
    let dose_range = db_stats::intervention_dose_range(&state.pool, user_id, substance).await?;

    let dose_range = match dose_range {
        Some(dr) => dr,
        None => {
            // No interventions found -- return empty result
            return Ok(Json(BeforeAfterResponse {
                intervention_substance: substance.to_string(),
                first_dose: None,
                last_dose: None,
                metric: metric_out,
                before: WindowStats {
                    mean: None,
                    std_dev: None,
                    n: 0,
                    points: vec![],
                },
                after: WindowStats {
                    mean: None,
                    std_dev: None,
                    n: 0,
                    points: vec![],
                },
                change_pct: None,
                p_value: None,
                significant: false,
                test_used: "welch_t".to_string(),
                warning: Some("no interventions found for this substance".to_string()),
            }));
        }
    };

    // Determine windows
    let before_start = dose_range.first_dose - Duration::days(body.before_days);
    let before_end = dose_range.first_dose;

    // If intervention is ongoing (last_dose within 7 days of now), use first_dose to now for after.
    let now = Utc::now();
    let ongoing = (now - dose_range.last_dose).num_days() <= 7;
    let (after_start, after_end) = if ongoing {
        (dose_range.first_dose, now)
    } else {
        (
            dose_range.last_dose,
            dose_range.last_dose + Duration::days(body.after_days),
        )
    };

    // Fetch metric data for both windows
    let before_series = db_explore::query_series(
        &state.pool,
        user_id,
        &metric,
        before_start,
        before_end,
        body.resolution,
    )
    .await?;
    let after_series = db_explore::query_series(
        &state.pool,
        user_id,
        &metric,
        after_start,
        after_end,
        body.resolution,
    )
    .await?;

    let before_vals: Vec<f64> = before_series.points.iter().map(|p| p.v).collect();
    let after_vals: Vec<f64> = after_series.points.iter().map(|p| p.v).collect();

    let before_stats = compute_window_stats(&before_series.points);
    let after_stats = compute_window_stats(&after_series.points);

    let insufficient = before_vals.len() < 3 || after_vals.len() < 3;

    let (p_value, significant, change_pct, warning) = if insufficient {
        (
            None,
            false,
            compute_change_pct(before_stats.mean, after_stats.mean),
            Some(
                "fewer than 3 data points in one or both windows — significance cannot be determined"
                    .to_string(),
            ),
        )
    } else {
        let (_t, p, _df) = stats::welch_t_test(&before_vals, &after_vals);
        let p_val = if p.is_nan() { None } else { Some(p) };
        let sig = p_val.is_some_and(|p| p < 0.05);
        (
            p_val,
            sig,
            compute_change_pct(before_stats.mean, after_stats.mean),
            None,
        )
    };

    Ok(Json(BeforeAfterResponse {
        intervention_substance: substance.to_string(),
        first_dose: Some(dose_range.first_dose),
        last_dose: Some(dose_range.last_dose),
        metric: metric_out,
        before: before_stats,
        after: after_stats,
        change_pct,
        p_value,
        significant,
        test_used: "welch_t".to_string(),
        warning,
    }))
}

/// POST /stats/correlate
pub async fn correlate(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<CorrelateRequest>,
) -> Result<Json<CorrelateResponse>, ApiError> {
    let metric_a = body.metric_a.parse()?;
    let metric_b = body.metric_b.parse()?;

    if body.start >= body.end {
        return Err(ApiError::BadRequest("start must be before end".to_string()));
    }

    let series_a = db_explore::query_series(
        &state.pool,
        user_id,
        &metric_a,
        body.start,
        body.end,
        body.resolution,
    )
    .await?;
    let series_b = db_explore::query_series(
        &state.pool,
        user_id,
        &metric_b,
        body.start,
        body.end,
        body.resolution,
    )
    .await?;

    // Align by timestamp
    let (aligned_a, aligned_b, timestamps) = align_series(&series_a.points, &series_b.points);

    let n = aligned_a.len();
    let (r, p, _n) = if n < 3 {
        (f64::NAN, f64::NAN, n)
    } else {
        match body.method {
            CorrelationMethod::Pearson => stats::pearson(&aligned_a, &aligned_b),
            CorrelationMethod::Spearman => stats::spearman(&aligned_a, &aligned_b),
        }
    };

    let r_out = if r.is_nan() { None } else { Some(r) };
    let p_out = if p.is_nan() { None } else { Some(p) };
    let significant = p_out.is_some_and(|p| p < 0.05);
    let interpretation = stats::interpret_correlation(r).to_string();

    let scatter: Vec<ScatterPoint> = (0..n)
        .map(|i| ScatterPoint {
            a: aligned_a[i],
            b: aligned_b[i],
            t: timestamps[i],
        })
        .collect();

    Ok(Json(CorrelateResponse {
        metric_a: MetricRefOut {
            source: body.metric_a.source,
            field: body.metric_a.field,
        },
        metric_b: MetricRefOut {
            source: body.metric_b.source,
            field: body.metric_b.field,
        },
        r: r_out,
        p_value: p_out,
        n,
        significant,
        method: body.method,
        interpretation,
        scatter,
    }))
}

/// POST /stats/lag-correlate
pub async fn lag_correlate(
    State(state): State<AppState>,
    AuthUser { id: user_id, .. }: AuthUser,
    Json(body): Json<LagCorrelateRequest>,
) -> Result<Json<LagCorrelateResponse>, ApiError> {
    let metric_a = body.metric_a.parse()?;
    let metric_b = body.metric_b.parse()?;

    if body.start >= body.end {
        return Err(ApiError::BadRequest("start must be before end".to_string()));
    }
    if body.max_lag_days < 1 || body.max_lag_days > 30 {
        return Err(ApiError::BadRequest(
            "max_lag_days must be between 1 and 30".to_string(),
        ));
    }

    // Fetch with extra margin for lag shifting
    let margin = Duration::days(body.max_lag_days);
    let series_a = db_explore::query_series(
        &state.pool,
        user_id,
        &metric_a,
        body.start - margin,
        body.end + margin,
        body.resolution,
    )
    .await?;
    let series_b = db_explore::query_series(
        &state.pool,
        user_id,
        &metric_b,
        body.start - margin,
        body.end + margin,
        body.resolution,
    )
    .await?;

    // Build lookup maps by date (truncated to day)
    let map_a: HashMap<i64, f64> = series_a
        .points
        .iter()
        .map(|p| (p.t.timestamp() / 86400, p.v))
        .collect();
    let map_b: HashMap<i64, f64> = series_b
        .points
        .iter()
        .map(|p| (p.t.timestamp() / 86400, p.v))
        .collect();

    let corr_fn = match body.method {
        CorrelationMethod::Pearson => stats::pearson,
        CorrelationMethod::Spearman => stats::spearman,
    };

    let mut lags = Vec::new();
    let mut best: Option<(i64, f64, Option<f64>)> = None;

    for lag in -body.max_lag_days..=body.max_lag_days {
        // Positive lag means B is shifted forward (lagged behind A).
        // For each day_key in A, pair with B at day_key + lag.
        let mut vals_a = Vec::new();
        let mut vals_b = Vec::new();

        for (&day_key, &va) in &map_a {
            if let Some(&vb) = map_b.get(&(day_key + lag)) {
                vals_a.push(va);
                vals_b.push(vb);
            }
        }

        let (r, p, n) = if vals_a.len() >= 3 {
            corr_fn(&vals_a, &vals_b)
        } else {
            (f64::NAN, f64::NAN, vals_a.len())
        };

        let r_out = if r.is_nan() { None } else { Some(r) };
        let p_out = if p.is_nan() { None } else { Some(p) };

        if let Some(r_val) = r_out {
            match &best {
                None => best = Some((lag, r_val, p_out)),
                Some((_, best_r, _)) => {
                    if r_val.abs() > best_r.abs() {
                        best = Some((lag, r_val, p_out));
                    }
                }
            }
        }

        lags.push(LagResult {
            lag,
            r: r_out,
            p_value: p_out,
            n,
        });
    }

    Ok(Json(LagCorrelateResponse {
        metric_a: MetricRefOut {
            source: body.metric_a.source,
            field: body.metric_a.field,
        },
        metric_b: MetricRefOut {
            source: body.metric_b.source,
            field: body.metric_b.field,
        },
        lags,
        best_lag: best.map(|(lag, r, p)| BestLag { lag, r, p_value: p }),
        method: body.method,
    }))
}

// ---- helpers ----

fn compute_window_stats(points: &[DataPoint]) -> WindowStats {
    let vals: Vec<f64> = points.iter().map(|p| p.v).collect();
    let time_vals: Vec<TimeValue> = points
        .iter()
        .map(|p| TimeValue { t: p.t, v: p.v })
        .collect();

    if vals.is_empty() {
        return WindowStats {
            mean: None,
            std_dev: None,
            n: 0,
            points: time_vals,
        };
    }

    WindowStats {
        mean: Some(stats::mean(&vals)),
        std_dev: Some(stats::std_dev(&vals)),
        n: vals.len(),
        points: time_vals,
    }
}

fn compute_change_pct(before_mean: Option<f64>, after_mean: Option<f64>) -> Option<f64> {
    match (before_mean, after_mean) {
        (Some(b), Some(a)) if b.abs() > f64::EPSILON => Some(((a - b) / b) * 100.0),
        _ => None,
    }
}

/// Align two time series by their timestamps. Only includes buckets present in both.
fn align_series(
    a: &[DataPoint],
    b: &[DataPoint],
) -> (Vec<f64>, Vec<f64>, Vec<chrono::DateTime<chrono::Utc>>) {
    let map_b: HashMap<i64, f64> = b.iter().map(|p| (p.t.timestamp(), p.v)).collect();

    let mut vals_a = Vec::new();
    let mut vals_b = Vec::new();
    let mut timestamps = Vec::new();

    for point in a {
        if let Some(&vb) = map_b.get(&point.t.timestamp()) {
            vals_a.push(point.v);
            vals_b.push(vb);
            timestamps.push(point.t);
        }
    }

    (vals_a, vals_b, timestamps)
}
