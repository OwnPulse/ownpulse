// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use sqlx::PgPool;

/// Insert a telemetry event into app_events. Fire-and-forget — caller spawns
/// this. `platform` is persisted as supplied by the caller (validated upstream
/// to be a known platform such as `"ios"` or `"web"`).
pub async fn insert_event(
    pool: &PgPool,
    event_type: &str,
    device_id: Option<&str>,
    payload: &serde_json::Value,
    app_version: Option<&str>,
    platform: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_events (event_type, device_id, payload, app_version, platform)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(event_type)
    .bind(device_id)
    .bind(payload)
    .bind(app_version)
    .bind(platform)
    .execute(pool)
    .await?;
    Ok(())
}

/// Aggregate health stats for the telemetry-ingest pipeline.
///
/// Contains only non-identifying aggregates: the number of `app_events`
/// received in the last 5 minutes and the timestamp of the most recent event
/// (across all time). No `device_id`, `payload`, `user_id`, or per-user data is
/// read or returned — this surfaces pipeline liveness only.
#[derive(Debug, Clone, Copy)]
pub struct PipelineStats {
    pub events_last_5m: i64,
    pub last_event_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Query the telemetry-ingest pipeline health aggregates.
pub async fn pipeline_stats(pool: &PgPool) -> Result<PipelineStats, sqlx::Error> {
    let row: (i64, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "SELECT
             COUNT(*) FILTER (WHERE created_at >= now() - interval '5 minutes') AS events_last_5m,
             MAX(created_at) AS last_event_at
         FROM app_events",
    )
    .fetch_one(pool)
    .await?;

    Ok(PipelineStats {
        events_last_5m: row.0,
        last_event_at: row.1,
    })
}
