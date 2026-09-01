// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Full JSON export of all user data, streamed as a single response body.

use axum::body::{Body, Bytes};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::calendar_day::CalendarDayRow;
use crate::models::checkin::CheckinRow;
use crate::models::genetics::GeneticRecordRow;
use crate::models::health_record::HealthRecordRow;
use crate::models::intervention::InterventionRow;
use crate::models::lab_result::LabResultRow;
use crate::models::observation::ObservationRow;
use crate::models::protocol::{ProtocolDoseRow, ProtocolLineRow, ProtocolRow, ProtocolRunRow};

/// Build a streaming JSON export body containing all data for the given user.
///
/// Fetches health_records, interventions, daily_checkins, lab_results,
/// observations, protocols, protocol_lines, protocol_runs, protocol_doses,
/// and calendar_days, then serialises the combined payload into a single
/// JSON document wrapped in `Body::from_stream`.
///
/// Sleep data has no separate table: it is stored as an `observations` row
/// with `type = 'sleep'` (see `routes/sleep.rs`), so it is already covered
/// by the `observations` array below — no additional query is needed.
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
         fasted, timing_relative_to, notes, healthkit_written, created_at, updated_at \
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
         source_id, loinc_code, uploaded_file_id, created_at \
         FROM lab_results WHERE user_id = $1 ORDER BY panel_date",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    // ObservationRow maps the DB column `type` to the struct field `obs_type`
    // via `#[sqlx(rename = "type")]`, so the SELECT must return a column
    // literally named `type` (not aliased) for FromRow to find it — an
    // `AS "obs_type"` alias here 500s any export with at least one
    // observation, since sqlx looks for a column called `type`.
    let observations = sqlx::query_as::<_, ObservationRow>(
        "SELECT id, user_id, type, name, start_time, \
         end_time, value, source, source_id, metadata, created_at \
         FROM observations WHERE user_id = $1 ORDER BY start_time",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    // Only include genetic records if the user has any (avoid empty array for non-uploaders)
    let genetic_records = sqlx::query_as::<_, GeneticRecordRow>(
        "SELECT id, user_id, source, rsid, chromosome, position, genotype, \
         uploaded_file_id, created_at \
         FROM genetic_records WHERE user_id = $1 ORDER BY chromosome, position",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    // protocols.user_id is nullable (templates have user_id = NULL), so
    // scoping by user_id here naturally excludes templates and other users'
    // protocols.
    let protocols = sqlx::query_as::<_, ProtocolRow>(
        "SELECT id, user_id, name, description, start_date, duration_days, \
         status, is_template, tags, source_url, share_token, share_expires_at, \
         created_at \
         FROM protocols WHERE user_id = $1 ORDER BY created_at",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let protocol_lines = sqlx::query_as::<_, ProtocolLineRow>(
        "SELECT pl.id, pl.protocol_id, pl.substance, pl.dose, pl.unit, pl.route, \
         pl.time_of_day, pl.schedule_pattern, pl.sort_order, pl.created_at \
         FROM protocol_lines pl \
         JOIN protocols p ON pl.protocol_id = p.id \
         WHERE p.user_id = $1 \
         ORDER BY pl.protocol_id, pl.sort_order",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let protocol_runs = sqlx::query_as::<_, ProtocolRunRow>(
        "SELECT id, protocol_id, user_id, start_date, status, notify, notify_time, \
         notify_times, repeat_reminders, repeat_interval_minutes, created_at \
         FROM protocol_runs WHERE user_id = $1 ORDER BY start_date",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    // Doses are scoped to the user via their line's protocol, since legacy
    // (pre-runs) doses have run_id = NULL and can't be scoped via
    // protocol_runs alone.
    let protocol_doses = sqlx::query_as::<_, ProtocolDoseRow>(
        "SELECT pd.id, pd.protocol_line_id, pd.day_number, pd.status, \
         pd.intervention_id, pd.logged_at, pd.run_id, pd.skip_reason \
         FROM protocol_doses pd \
         JOIN protocol_lines pl ON pd.protocol_line_id = pl.id \
         JOIN protocols p ON pl.protocol_id = p.id \
         WHERE p.user_id = $1 \
         ORDER BY pd.logged_at",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let calendar_days = sqlx::query_as::<_, CalendarDayRow>(
        "SELECT id, user_id, date, meeting_count, meeting_minutes, synced_at \
         FROM calendar_days WHERE user_id = $1 ORDER BY date",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut payload = serde_json::json!({
        "schema_version": "0.3.0",
        "exported_at": Utc::now(),
        "health_records": health_records,
        "interventions": interventions,
        "daily_checkins": daily_checkins,
        "lab_results": lab_results,
        "observations": observations,
        "protocols": protocols,
        "protocol_lines": protocol_lines,
        "protocol_runs": protocol_runs,
        "protocol_doses": protocol_doses,
        "calendar_days": calendar_days,
    });

    if !genetic_records.is_empty() {
        payload["genetic_records"] = serde_json::to_value(&genetic_records)
            .expect("serialization of genetic records should not fail");
    }

    let json_bytes =
        serde_json::to_vec(&payload).expect("serialization of export payload should not fail");

    let stream =
        futures::stream::once(async move { Ok::<Bytes, std::io::Error>(Bytes::from(json_bytes)) });

    Ok(Body::from_stream(stream))
}
