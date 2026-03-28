// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::explore::{
    CalendarField, CheckinField, DataPoint, HealthRecordField, MetricSource, Resolution,
    SeriesResponse, SleepField,
};

/// Row returned by aggregation queries.
#[derive(sqlx::FromRow)]
struct AggRow {
    bucket: Option<DateTime<Utc>>,
    avg_val: Option<f64>,
    cnt: Option<i64>,
}

/// Query aggregated time-series data for a validated `MetricSource`.
pub async fn query_series(
    pool: &PgPool,
    user_id: Uuid,
    metric: &MetricSource,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    resolution: Resolution,
) -> Result<SeriesResponse, sqlx::Error> {
    let interval = resolution.pg_interval();

    let (source_str, field_str, unit, points) = match metric {
        MetricSource::HealthRecord(field) => {
            let rows = query_health_record(pool, user_id, field, start, end, interval).await?;
            (
                "health_records".to_string(),
                field.record_type().to_string(),
                field.unit().to_string(),
                agg_to_points(rows),
            )
        }
        MetricSource::Checkin(field) => {
            let rows = query_checkin(pool, user_id, field, start, end, interval).await?;
            (
                "checkins".to_string(),
                field.column().to_string(),
                "score".to_string(),
                agg_to_points(rows),
            )
        }
        MetricSource::Lab(marker) => {
            let rows = query_lab(pool, user_id, marker, start, end, interval).await?;
            (
                "labs".to_string(),
                marker.clone(),
                "value".to_string(),
                agg_to_points(rows),
            )
        }
        MetricSource::Calendar(field) => {
            let rows = query_calendar(pool, user_id, field, start, end, interval).await?;
            (
                "calendar".to_string(),
                field.column().to_string(),
                field.unit().to_string(),
                agg_to_points(rows),
            )
        }
        MetricSource::Sleep(field) => {
            let rows = query_sleep(pool, user_id, field, start, end, interval).await?;
            (
                "sleep".to_string(),
                field.json_key().to_string(),
                field.unit().to_string(),
                agg_to_points(rows),
            )
        }
    };

    Ok(SeriesResponse {
        source: source_str,
        field: field_str,
        unit,
        points,
    })
}

fn agg_to_points(rows: Vec<AggRow>) -> Vec<DataPoint> {
    rows.into_iter()
        .filter_map(|r| {
            Some(DataPoint {
                t: r.bucket?,
                v: r.avg_val?,
                n: r.cnt.unwrap_or(0),
            })
        })
        .collect()
}

async fn query_health_record(
    pool: &PgPool,
    user_id: Uuid,
    field: &HealthRecordField,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    interval: &str,
) -> Result<Vec<AggRow>, sqlx::Error> {
    let record_type = field.record_type();
    sqlx::query_as::<_, AggRow>(&format!(
        "SELECT date_trunc('{interval}', start_time) AS bucket,
                AVG(value) AS avg_val,
                COUNT(*) AS cnt
         FROM health_records
         WHERE user_id = $1 AND record_type = $2
           AND start_time >= $3 AND start_time <= $4
           AND value IS NOT NULL
         GROUP BY bucket
         ORDER BY bucket ASC"
    ))
    .bind(user_id)
    .bind(record_type)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
}

// Safety: the column name is sourced from a Rust enum variant (CheckinField),
// not from user input. The enum's `column()` method returns one of five
// hardcoded string literals: "energy", "mood", "focus", "recovery", "libido".
// This is safe to interpolate into SQL via format!.
async fn query_checkin(
    pool: &PgPool,
    user_id: Uuid,
    field: &CheckinField,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    interval: &str,
) -> Result<Vec<AggRow>, sqlx::Error> {
    let col = field.column();
    sqlx::query_as::<_, AggRow>(&format!(
        "SELECT date_trunc('{interval}', date::timestamptz) AS bucket,
                AVG({col}::double precision) AS avg_val,
                COUNT(*) AS cnt
         FROM daily_checkins
         WHERE user_id = $1
           AND date >= ($2::timestamptz)::date
           AND date <= ($3::timestamptz)::date
           AND {col} IS NOT NULL
         GROUP BY bucket
         ORDER BY bucket ASC"
    ))
    .bind(user_id)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
}

async fn query_lab(
    pool: &PgPool,
    user_id: Uuid,
    marker: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    interval: &str,
) -> Result<Vec<AggRow>, sqlx::Error> {
    sqlx::query_as::<_, AggRow>(&format!(
        "SELECT date_trunc('{interval}', panel_date::timestamptz) AS bucket,
                AVG(value) AS avg_val,
                COUNT(*) AS cnt
         FROM lab_results
         WHERE user_id = $1 AND marker = $2
           AND panel_date >= ($3::timestamptz)::date
           AND panel_date <= ($4::timestamptz)::date
         GROUP BY bucket
         ORDER BY bucket ASC"
    ))
    .bind(user_id)
    .bind(marker)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
}

// Safety: the column name comes from CalendarField enum — see comment on query_checkin.
async fn query_calendar(
    pool: &PgPool,
    user_id: Uuid,
    field: &CalendarField,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    interval: &str,
) -> Result<Vec<AggRow>, sqlx::Error> {
    let col = field.column();
    sqlx::query_as::<_, AggRow>(&format!(
        "SELECT date_trunc('{interval}', date::timestamptz) AS bucket,
                AVG({col}::double precision) AS avg_val,
                COUNT(*) AS cnt
         FROM calendar_days
         WHERE user_id = $1
           AND date >= ($2::timestamptz)::date
           AND date <= ($3::timestamptz)::date
         GROUP BY bucket
         ORDER BY bucket ASC"
    ))
    .bind(user_id)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
}

// Safety: the JSON key comes from SleepField enum — see comment on query_checkin.
async fn query_sleep(
    pool: &PgPool,
    user_id: Uuid,
    field: &SleepField,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    interval: &str,
) -> Result<Vec<AggRow>, sqlx::Error> {
    let json_key = field.json_key();
    sqlx::query_as::<_, AggRow>(&format!(
        "SELECT date_trunc('{interval}', start_time) AS bucket,
                AVG((value->>'{json_key}')::double precision) AS avg_val,
                COUNT(*) AS cnt
         FROM observations
         WHERE user_id = $1 AND type = 'sleep'
           AND start_time >= $2 AND start_time <= $3
           AND value->>'{json_key}' IS NOT NULL
         GROUP BY bucket
         ORDER BY bucket ASC"
    ))
    .bind(user_id)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
}

/// Get distinct lab markers for a user (for the metrics picker).
pub async fn distinct_lab_markers(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT marker FROM lab_results WHERE user_id = $1 ORDER BY marker",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|(m,)| m).collect())
}
