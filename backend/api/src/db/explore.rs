// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::explore::{
    Aggregation, CalendarField, CheckinField, DataPoint, HealthRecordField, InterventionMarker,
    MetricSource, ObserverPollField, Resolution, SeriesResponse, SleepField,
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
            let rows = match field.aggregation() {
                Aggregation::Avg => {
                    query_health_record_avg(pool, user_id, field, start, end, interval).await?
                }
                Aggregation::Sum => {
                    query_health_record_sum(pool, user_id, field, start, end, interval).await?
                }
                Aggregation::SleepDuration => {
                    query_health_record_sleep_duration(pool, user_id, field, start, end, interval)
                        .await?
                }
                Aggregation::CountEvents => {
                    query_health_record_count(pool, user_id, field, start, end, interval).await?
                }
            };
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
        MetricSource::ObserverPoll(field) => {
            let rows = query_observer_poll(pool, user_id, field, start, end, interval).await?;
            (
                "observer_polls".to_string(),
                format!("{}:{}", field.poll_id, field.dimension),
                "score".to_string(),
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

async fn query_health_record_avg(
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

async fn query_health_record_sum(
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
                SUM(value) AS avg_val,
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

/// Sleep/mindful duration: sum of segment durations in minutes for sleep
/// categories (1=InBed, 3=Core, 4=Deep, 5=REM) where end_time is present.
async fn query_health_record_sleep_duration(
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
                SUM(EXTRACT(EPOCH FROM (end_time - start_time)) / 60) AS avg_val,
                COUNT(*) AS cnt
         FROM health_records
         WHERE user_id = $1 AND record_type = $2
           AND start_time >= $3 AND start_time <= $4
           AND end_time IS NOT NULL
           AND value IN (1, 3, 4, 5)
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

/// Count events per bucket (no value aggregation — just COUNT(*)).
async fn query_health_record_count(
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
                COUNT(*)::double precision AS avg_val,
                COUNT(*) AS cnt
         FROM health_records
         WHERE user_id = $1 AND record_type = $2
           AND start_time >= $3 AND start_time <= $4
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

/// Query observer poll responses, averaging across all observers for a single
/// dimension.  The poll must be owned by `user_id`.
async fn query_observer_poll(
    pool: &PgPool,
    user_id: Uuid,
    field: &ObserverPollField,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    interval: &str,
) -> Result<Vec<AggRow>, sqlx::Error> {
    let dimension = &field.dimension;
    // Safety: `dimension` comes from user input but is used as a JSONB key via
    // the `->>` operator with a bind parameter style (format! interpolation here
    // because `sqlx` cannot bind a key name). We validate it is non-empty and
    // alphanumeric+underscore to prevent injection.
    //
    // We also verify ownership inline (AND p.user_id = $1) so a user cannot
    // query another user's poll data.
    sqlx::query_as::<_, AggRow>(&format!(
        "SELECT date_trunc('{interval}', r.date::timestamptz) AS bucket,
                AVG((r.scores->>'{dimension}')::double precision) AS avg_val,
                COUNT(*) AS cnt
         FROM observer_responses r
         JOIN observer_polls p ON p.id = r.poll_id
         WHERE r.poll_id = $1
           AND p.user_id = $2
           AND p.deleted_at IS NULL
           AND r.date >= ($3::timestamptz)::date
           AND r.date <= ($4::timestamptz)::date
           AND r.scores->>'{dimension}' IS NOT NULL
         GROUP BY bucket
         ORDER BY bucket ASC"
    ))
    .bind(field.poll_id)
    .bind(user_id)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
}

/// Query intervention markers for a date range.
pub async fn intervention_markers(
    pool: &PgPool,
    user_id: Uuid,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<InterventionMarker>, sqlx::Error> {
    sqlx::query_as::<_, InterventionMarker>(
        "SELECT administered_at AS t, substance, dose, unit, route
         FROM interventions
         WHERE user_id = $1
           AND administered_at >= $2
           AND administered_at <= $3
         ORDER BY administered_at ASC",
    )
    .bind(user_id)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await
}

/// Fetch active observer polls owned by a user (for the metrics picker).
pub async fn user_observer_polls(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<(Uuid, String, serde_json::Value)>, sqlx::Error> {
    sqlx::query_as::<_, (Uuid, String, serde_json::Value)>(
        "SELECT id, name, dimensions
         FROM observer_polls
         WHERE user_id = $1 AND deleted_at IS NULL
         ORDER BY created_at DESC",
    )
    .bind(user_id)
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
