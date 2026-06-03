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
