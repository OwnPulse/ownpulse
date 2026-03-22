// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Full JSON export of all user data, streamed as a single response body.

use axum::body::{Body, Bytes};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::checkin::CheckinRow;
use crate::models::health_record::HealthRecordRow;
use crate::models::intervention::InterventionRow;
use crate::models::lab_result::LabResultRow;
use crate::models::observation::ObservationRow;

/// Build a streaming JSON export body containing all data for the given user.
///
/// Fetches health_records, interventions, daily_checkins, lab_results, and
/// observations, then serialises the combined payload into a single JSON
/// document wrapped in `Body::from_stream`.
pub async fn stream_json_export(pool: &PgPool, user_id: Uuid) -> Result<Body, sqlx::Error> {
    let health_records = sqlx::query_as::<_, HealthRecordRow>(
        "SELECT id, user_id, source, record_type, value, unit, start_time, \
         end_time, metadata, source_id, source_instance, duplicate_of, \
         healthkit_written, created_at \
         FROM health_records WHERE user_id = $1 ORDER BY start_time",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let interventions = sqlx::query_as::<_, InterventionRow>(
        "SELECT id, user_id, substance, dose, unit, route, administered_at, \
         fasted, timing_relative_to, notes, healthkit_written, created_at \
         FROM interventions WHERE user_id = $1 ORDER BY administered_at",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let daily_checkins = sqlx::query_as::<_, CheckinRow>(
        "SELECT id, user_id, date, energy, mood, focus, recovery, libido, \
         notes, created_at \
         FROM daily_checkins WHERE user_id = $1 ORDER BY date",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let lab_results = sqlx::query_as::<_, LabResultRow>(
        "SELECT id, user_id, panel_date, lab_name, marker, value, unit, \
         reference_low, reference_high, out_of_range, source, \
         uploaded_file_id, created_at \
         FROM lab_results WHERE user_id = $1 ORDER BY panel_date",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let observations = sqlx::query_as::<_, ObservationRow>(
        "SELECT id, user_id, type as \"obs_type\", name, start_time, \
         end_time, value, source, metadata, created_at \
         FROM observations WHERE user_id = $1 ORDER BY start_time",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let payload = serde_json::json!({
        "schema_version": "0.1.0",
        "exported_at": Utc::now(),
        "health_records": health_records,
        "interventions": interventions,
        "daily_checkins": daily_checkins,
        "lab_results": lab_results,
        "observations": observations,
    });

    let json_bytes =
        serde_json::to_vec(&payload).expect("serialization of export payload should not fail");

    let stream =
        futures::stream::once(async move { Ok::<Bytes, std::io::Error>(Bytes::from(json_bytes)) });

    Ok(Body::from_stream(stream))
}
