// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{Duration, NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::Config;
use crate::dose_status;
use crate::models::protocol::{
    ActiveSubstanceItem, AdherenceResponse, CreateProtocol, CreateRunRequest, LineAdherence,
    LineAdherenceRow, LogDoseRequest, MissedDoseItem, NotificationPreferencesRow, ProtocolDoseRow,
    ProtocolExport, ProtocolLineExport, ProtocolLineResponse, ProtocolLineRow, ProtocolListItem,
    ProtocolResponse, ProtocolRow, ProtocolRunRow, PushTokenRow, RegisterPushTokenRequest,
    RunDoseItem, RunResponse, SkipDoseRequest, TemplateListItem, TodaysDoseItem,
    UpdateNotificationPreferences, UpdateProtocol, UpdateRunRequest,
};

/// Insert a new protocol with its lines in a transaction.
/// `start_date` is now optional (protocols are recipes).
pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    req: &CreateProtocol,
) -> Result<ProtocolRow, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let protocol = sqlx::query_as::<_, ProtocolRow>(
        "INSERT INTO protocols (user_id, name, description, start_date, duration_days)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, user_id, name, description, start_date, duration_days,
                   status, is_template, tags, source_url,
                   share_token, share_expires_at, created_at",
    )
    .bind(user_id)
    .bind(&req.name)
    .bind(&req.description)
    .bind(req.start_date)
    .bind(req.duration_days)
    .fetch_one(&mut *tx)
    .await?;

    for line in &req.lines {
        let pattern_json = serde_json::to_value(&line.schedule_pattern)
            .unwrap_or_else(|_| serde_json::Value::Array(vec![]));

        sqlx::query(
            "INSERT INTO protocol_lines
                (protocol_id, substance, dose, unit, route, time_of_day, schedule_pattern, sort_order)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(protocol.id)
        .bind(&line.substance)
        .bind(line.dose)
        .bind(&line.unit)
        .bind(&line.route)
        .bind(&line.time_of_day)
        .bind(&pattern_json)
        .bind(line.sort_order)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(protocol)
}

/// List protocols for a user (recipes). Progress is computed from active runs.
pub async fn list(pool: &PgPool, user_id: Uuid) -> Result<Vec<ProtocolListItem>, sqlx::Error> {
    sqlx::query_as::<_, ProtocolListItem>(
        "SELECT
            p.id,
            p.name,
            p.status,
            p.start_date,
            p.duration_days,
            p.is_template,
            p.tags,
            COALESCE(
                (SELECT
                    CASE
                        WHEN r.status = 'completed' THEN 100.0
                        WHEN CURRENT_DATE < r.start_date THEN 0.0
                        ELSE LEAST(
                            100.0,
                            (CURRENT_DATE - r.start_date)::double precision / p.duration_days * 100.0
                        )
                    END
                 FROM protocol_runs r
                 WHERE r.protocol_id = p.id AND r.status = 'active'
                 ORDER BY r.created_at DESC
                 LIMIT 1),
                0.0
            ) AS progress_pct,
            (
                SELECT pl.substance
                FROM protocol_lines pl
                LEFT JOIN protocol_runs r
                    ON r.protocol_id = p.id AND r.status = 'active'
                LEFT JOIN protocol_doses pd
                    ON pd.protocol_line_id = pl.id
                    AND pd.run_id = r.id
                    AND pd.day_number = (CURRENT_DATE - r.start_date)
                WHERE pl.protocol_id = p.id
                    AND r.id IS NOT NULL
                    AND pd.id IS NULL
                    AND (CURRENT_DATE - r.start_date) >= 0
                    AND (CURRENT_DATE - r.start_date) < p.duration_days
                ORDER BY pl.sort_order
                LIMIT 1
            ) AS next_dose,
            p.created_at
         FROM protocols p
         WHERE p.user_id = $1
         ORDER BY p.created_at DESC
         LIMIT 100",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Get a full protocol with lines, doses, and runs.
pub async fn get_by_id(
    pool: &PgPool,
    protocol_id: Uuid,
    user_id: Uuid,
) -> Result<ProtocolResponse, sqlx::Error> {
    let protocol = sqlx::query_as::<_, ProtocolRow>(
        "SELECT id, user_id, name, description, start_date, duration_days,
                status, is_template, tags, source_url,
                share_token, share_expires_at, created_at
         FROM protocols
         WHERE id = $1 AND user_id = $2",
    )
    .bind(protocol_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let runs = list_runs_for_protocol(pool, protocol_id, user_id).await?;
    // Scope the dose grid to a single run: the active one, or the most
    // recently created run if none is active (see `resolve_current_run_id`).
    let current_run_id = resolve_current_run_id(pool, protocol_id).await?;
    let lines = fetch_lines_with_doses(pool, protocol_id, current_run_id).await?;

    Ok(build_response(protocol, lines, runs))
}

/// Update a protocol's mutable fields.
pub async fn update(
    pool: &PgPool,
    protocol_id: Uuid,
    user_id: Uuid,
    req: &UpdateProtocol,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE protocols
         SET name = COALESCE($3, name),
             description = COALESCE($4, description),
             status = COALESCE($5, status)
         WHERE id = $1 AND user_id = $2",
    )
    .bind(protocol_id)
    .bind(user_id)
    .bind(&req.name)
    .bind(&req.description)
    .bind(&req.status)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Delete a protocol. Returns true if a row was actually deleted.
pub async fn delete(pool: &PgPool, protocol_id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM protocols WHERE id = $1 AND user_id = $2")
        .bind(protocol_id)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

// --- Protocol Runs ---

/// Create a new run for a protocol.
pub async fn create_run(
    pool: &PgPool,
    protocol_id: Uuid,
    user_id: Uuid,
    req: &CreateRunRequest,
) -> Result<ProtocolRunRow, sqlx::Error> {
    // Verify user owns the protocol
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM protocols WHERE id = $1 AND user_id = $2")
        .bind(protocol_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

    let start_date = req.start_date.unwrap_or_else(|| Utc::now().date_naive());
    let notify = req.notify.unwrap_or(false);
    let notify_times = req
        .notify_times
        .as_ref()
        .map(|t| serde_json::to_value(t).unwrap_or_default());
    let repeat_reminders = req.repeat_reminders.unwrap_or(false);

    sqlx::query_as::<_, ProtocolRunRow>(
        "INSERT INTO protocol_runs
            (protocol_id, user_id, start_date, notify, notify_time, notify_times,
             repeat_reminders, repeat_interval_minutes)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, protocol_id, user_id, start_date, status, notify, notify_time,
                   notify_times, repeat_reminders, repeat_interval_minutes, created_at",
    )
    .bind(protocol_id)
    .bind(user_id)
    .bind(start_date)
    .bind(notify)
    .bind(&req.notify_time)
    .bind(&notify_times)
    .bind(repeat_reminders)
    .bind(req.repeat_interval_minutes)
    .fetch_one(pool)
    .await
}

/// List runs for a specific protocol.
async fn list_runs_for_protocol(
    pool: &PgPool,
    protocol_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<RunResponse>, sqlx::Error> {
    let runs = sqlx::query_as::<_, ProtocolRunRow>(
        "SELECT id, protocol_id, user_id, start_date, status, notify, notify_time,
                notify_times, repeat_reminders, repeat_interval_minutes, created_at
         FROM protocol_runs
         WHERE protocol_id = $1 AND user_id = $2
         ORDER BY created_at DESC",
    )
    .bind(protocol_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let duration: Option<i32> =
        sqlx::query_scalar("SELECT duration_days FROM protocols WHERE id = $1")
            .bind(protocol_id)
            .fetch_optional(pool)
            .await?;

    Ok(runs
        .into_iter()
        .map(|r| run_to_response(r, None, duration))
        .collect())
}

/// List runs for a protocol (public API endpoint).
pub async fn list_runs(
    pool: &PgPool,
    protocol_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<RunResponse>, sqlx::Error> {
    list_runs_for_protocol(pool, protocol_id, user_id).await
}

/// List all active runs across all protocols for a user.
pub async fn list_active_runs(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<RunResponse>, sqlx::Error> {
    // Use a struct to capture the joined data including dose counts
    #[derive(sqlx::FromRow)]
    struct RunWithProtocol {
        id: Uuid,
        protocol_id: Uuid,
        user_id: Uuid,
        start_date: NaiveDate,
        status: String,
        notify: bool,
        notify_time: Option<String>,
        notify_times: Option<serde_json::Value>,
        repeat_reminders: bool,
        repeat_interval_minutes: Option<i32>,
        created_at: chrono::DateTime<Utc>,
        protocol_name: String,
        duration_days: i32,
        doses_today: i64,
        doses_completed_today: i64,
        scheduled_so_far: i64,
        completed_so_far: i64,
        doses_missed: i64,
    }

    // The `scheduled_so_far` / `completed_so_far` / `doses_missed` columns
    // implement the same canonical dose-status rule as
    // `crate::dose_status::compute_dose_status`, expressed in SQL so all
    // active runs are scored in one query (no N+1 across runs). See that
    // module's doc comment for the rule itself.
    let rows = sqlx::query_as::<_, RunWithProtocol>(
        "SELECT r.id, r.protocol_id, r.user_id, r.start_date, r.status,
                r.notify, r.notify_time, r.notify_times,
                r.repeat_reminders, r.repeat_interval_minutes, r.created_at,
                p.name AS protocol_name, p.duration_days,
                COALESCE((
                    SELECT COUNT(*)
                    FROM protocol_lines pl
                    WHERE pl.protocol_id = p.id
                      AND (CURRENT_DATE - r.start_date) >= 0
                      AND (CURRENT_DATE - r.start_date) < p.duration_days
                      AND (pl.schedule_pattern->((CURRENT_DATE - r.start_date)::int))::text = 'true'
                ), 0) AS doses_today,
                COALESCE((
                    SELECT COUNT(*)
                    FROM protocol_lines pl
                    JOIN protocol_doses pd
                        ON pd.protocol_line_id = pl.id
                        AND pd.run_id = r.id
                        AND pd.day_number = (CURRENT_DATE - r.start_date)
                        AND pd.status = 'completed'
                    WHERE pl.protocol_id = p.id
                      AND (CURRENT_DATE - r.start_date) >= 0
                      AND (CURRENT_DATE - r.start_date) < p.duration_days
                      AND (pl.schedule_pattern->((CURRENT_DATE - r.start_date)::int))::text = 'true'
                ), 0) AS doses_completed_today,
                COALESCE((
                    SELECT COUNT(*)
                    FROM protocol_lines pl
                    CROSS JOIN LATERAL generate_series(
                        0, LEAST((CURRENT_DATE - r.start_date)::int, p.duration_days - 1)
                    ) AS gs(day_number)
                    WHERE pl.protocol_id = p.id
                      AND (pl.schedule_pattern->gs.day_number)::text = 'true'
                ), 0) AS scheduled_so_far,
                COALESCE((
                    SELECT COUNT(*)
                    FROM protocol_doses pd
                    JOIN protocol_lines pl ON pl.id = pd.protocol_line_id
                    WHERE pl.protocol_id = p.id
                      AND pd.run_id = r.id
                      AND pd.status = 'completed'
                      AND pd.day_number <= LEAST((CURRENT_DATE - r.start_date)::int, p.duration_days - 1)
                ), 0) AS completed_so_far,
                COALESCE((
                    SELECT COUNT(*)
                    FROM protocol_lines pl
                    CROSS JOIN LATERAL generate_series(
                        0, LEAST((CURRENT_DATE - r.start_date)::int - 1, p.duration_days - 1)
                    ) AS gs(day_number)
                    WHERE pl.protocol_id = p.id
                      AND (pl.schedule_pattern->gs.day_number)::text = 'true'
                      AND NOT EXISTS (
                          SELECT 1 FROM protocol_doses pd
                          WHERE pd.protocol_line_id = pl.id
                            AND pd.run_id = r.id
                            AND pd.day_number = gs.day_number
                      )
                ), 0) AS doses_missed
         FROM protocol_runs r
         JOIN protocols p ON p.id = r.protocol_id
         WHERE r.user_id = $1 AND r.status = 'active'
         ORDER BY r.created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let run = ProtocolRunRow {
                id: r.id,
                protocol_id: r.protocol_id,
                user_id: r.user_id,
                start_date: r.start_date,
                status: r.status,
                notify: r.notify,
                notify_time: r.notify_time,
                notify_times: r.notify_times,
                repeat_reminders: r.repeat_reminders,
                repeat_interval_minutes: r.repeat_interval_minutes,
                created_at: r.created_at,
            };
            let mut resp = run_to_response(run, Some(r.protocol_name), Some(r.duration_days));
            resp.doses_today = r.doses_today;
            resp.doses_completed_today = r.doses_completed_today;
            resp.doses_missed = r.doses_missed;
            resp.adherence_pct = if r.scheduled_so_far == 0 {
                None
            } else {
                Some(r.completed_so_far as f64 / r.scheduled_so_far as f64 * 100.0)
            };
            resp
        })
        .collect())
}

/// Update a run's status and/or notification settings.
pub async fn update_run(
    pool: &PgPool,
    run_id: Uuid,
    user_id: Uuid,
    req: &UpdateRunRequest,
) -> Result<bool, sqlx::Error> {
    let notify_times = req
        .notify_times
        .as_ref()
        .map(|t| serde_json::to_value(t).unwrap_or_default());

    let result = sqlx::query(
        "UPDATE protocol_runs
         SET status = COALESCE($3, status),
             notify = COALESCE($4, notify),
             notify_time = COALESCE($5, notify_time),
             notify_times = COALESCE($6, notify_times),
             repeat_reminders = COALESCE($7, repeat_reminders),
             repeat_interval_minutes = COALESCE($8, repeat_interval_minutes)
         WHERE id = $1 AND user_id = $2",
    )
    .bind(run_id)
    .bind(user_id)
    .bind(&req.status)
    .bind(req.notify)
    .bind(&req.notify_time)
    .bind(&notify_times)
    .bind(req.repeat_reminders)
    .bind(req.repeat_interval_minutes)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Errors from logging a dose that need a distinct HTTP status (as opposed
/// to plain [`sqlx::Error`], which mostly maps to 404/409/500).
#[derive(Debug, thiserror::Error)]
pub enum DoseLogError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    /// Invalid input — maps to 400 Bad Request.
    #[error("{0}")]
    Invalid(String),
}

/// Derive the default local time-of-day for a dose logged without an
/// explicit `administered_at`: "AM" -> 08:00, "PM" -> 20:00, anything else
/// (including unset) -> 12:00. Interpreted in the caller's `tz_offset_minutes`
/// by [`log_dose_core`].
fn default_dose_time(time_of_day: Option<&str>) -> chrono::NaiveTime {
    let (h, m, s) = match time_of_day {
        Some(t) if t.eq_ignore_ascii_case("AM") => (8, 0, 0),
        Some(t) if t.eq_ignore_ascii_case("PM") => (20, 0, 0),
        _ => (12, 0, 0),
    };
    chrono::NaiveTime::from_hms_opt(h, m, s).unwrap_or_default()
}

/// The calendar date a UTC instant falls on in a given `tz_offset_minutes`
/// (positive = east of UTC), for date-only comparisons.
fn local_date(ts: chrono::DateTime<Utc>, tz_offset_minutes: i32) -> NaiveDate {
    (ts + chrono::Duration::minutes(i64::from(tz_offset_minutes))).date_naive()
}

/// Resolve which run a legacy protocol-level dose should be attached to:
/// the protocol's active run if one exists, otherwise its most recently
/// created run, otherwise `None` (a truly run-less protocol). This is the
/// same active-else-most-recent choice `get_by_id`/`get_shared` use to pick
/// which run's doses to display, so a legacy-logged dose is always visible
/// on the run-scoped dose grid instead of being invisible or colliding with
/// a NULL-run row.
async fn resolve_current_run_id<'e, E>(
    executor: E,
    protocol_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM protocol_runs
         WHERE protocol_id = $1
         ORDER BY (status = 'active') DESC, created_at DESC
         LIMIT 1",
    )
    .bind(protocol_id)
    .fetch_optional(executor)
    .await
}

/// Shared core of dose logging, used by both `log_dose_on_run` (explicit
/// `run_id`, must be active) and the legacy protocol-level `log_dose`
/// (`run_id` resolved via [`resolve_current_run_id`]) so notes,
/// `administered_at`/timezone handling, and validation live in one place
/// instead of drifting between the two route families. `run_id` is `None`
/// only when the protocol has no runs at all.
async fn log_dose_core(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    protocol_id: Uuid,
    run_id: Option<Uuid>,
    start_date: NaiveDate,
    duration_days: i32,
    req: &LogDoseRequest,
) -> Result<ProtocolDoseRow, DoseLogError> {
    // 1. Get the protocol_line (verify it belongs to the protocol)
    let line = sqlx::query_as::<_, ProtocolLineRow>(
        "SELECT id, protocol_id, substance, dose, unit, route, time_of_day,
                schedule_pattern, sort_order, created_at
         FROM protocol_lines
         WHERE id = $1 AND protocol_id = $2",
    )
    .bind(req.protocol_line_id)
    .bind(protocol_id)
    .fetch_one(&mut **tx)
    .await?;

    // 2. Verify the day_number is valid and schedule_pattern[day_number] is true
    let pattern = line
        .schedule_pattern
        .as_array()
        .ok_or(sqlx::Error::RowNotFound)?;

    if req.day_number < 0 || req.day_number >= duration_days {
        return Err(sqlx::Error::RowNotFound.into());
    }

    let scheduled = pattern
        .get(req.day_number as usize)
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !scheduled {
        return Err(sqlx::Error::RowNotFound.into());
    }

    let tz_offset = req.tz_offset_minutes.unwrap_or(0);
    if !(-840..=840).contains(&tz_offset) {
        return Err(DoseLogError::Invalid(
            "tz_offset_minutes must be between -840 and 840".to_string(),
        ));
    }

    // 3. Reject logging more than a day ahead of "today". A single day of
    // tolerance absorbs timezone skew — a user east of UTC may legitimately
    // be logging "their today" while it's still tomorrow in UTC. "Today" is
    // read from Postgres rather than `Utc::now()` so this shares one clock
    // with the rest of the dose logic (SQL `CURRENT_DATE` elsewhere), even
    // on a self-hosted deployment with a non-UTC Postgres `TimeZone`.
    let dose_date = start_date + chrono::Duration::days(i64::from(req.day_number));
    let today: NaiveDate = sqlx::query_scalar("SELECT CURRENT_DATE")
        .fetch_one(&mut **tx)
        .await?;
    if dose_date > today + chrono::Duration::days(1) {
        return Err(DoseLogError::Invalid(format!(
            "cannot log a dose for day {} ({dose_date}) — that day hasn't happened yet",
            req.day_number
        )));
    }

    // 4. Resolve the intervention timestamp: an explicit `administered_at`
    // must fall within a day of the calendar date of this dose (evaluated in
    // `tz_offset_minutes`, absorbing the same timezone skew as above);
    // otherwise derive a default from the line's time_of_day, interpreted in
    // that same offset.
    let administered_at_utc = match req.administered_at {
        Some(ts) => {
            let ts_local_date = local_date(ts, tz_offset);
            if (ts_local_date - dose_date).num_days().abs() > 1 {
                return Err(DoseLogError::Invalid(format!(
                    "administered_at must fall within a day of {dose_date} (day {} of this run)",
                    req.day_number
                )));
            }
            ts
        }
        None => {
            let local_dt = dose_date.and_time(default_dose_time(line.time_of_day.as_deref()));
            (local_dt - chrono::Duration::minutes(i64::from(tz_offset))).and_utc()
        }
    };

    // 5. Create an intervention record
    let intervention_id: Uuid = sqlx::query_scalar(
        "INSERT INTO interventions
            (user_id, substance, dose, unit, route, administered_at, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id",
    )
    .bind(user_id)
    .bind(&line.substance)
    .bind(line.dose)
    .bind(&line.unit)
    .bind(&line.route)
    .bind(administered_at_utc)
    .bind(&req.notes)
    .fetch_one(&mut **tx)
    .await?;

    // 6. Insert protocol_dose, scoped to `run_id` (NULL only for a protocol
    // with no runs at all).
    let dose = sqlx::query_as::<_, ProtocolDoseRow>(
        "INSERT INTO protocol_doses (protocol_line_id, day_number, status, intervention_id, run_id)
         VALUES ($1, $2, 'completed', $3, $4)
         RETURNING id, protocol_line_id, day_number, status, intervention_id, logged_at,
                   run_id, skip_reason",
    )
    .bind(req.protocol_line_id)
    .bind(req.day_number)
    .bind(intervention_id)
    .bind(run_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(dose)
}

/// Log a dose on a run.
pub async fn log_dose_on_run(
    pool: &PgPool,
    user_id: Uuid,
    run_id: Uuid,
    req: &LogDoseRequest,
    _config: &Config,
) -> Result<ProtocolDoseRow, DoseLogError> {
    let mut tx = pool.begin().await?;

    // Verify user owns the run and get protocol info
    #[derive(sqlx::FromRow)]
    struct RunInfo {
        protocol_id: Uuid,
        start_date: NaiveDate,
    }

    let run = sqlx::query_as::<_, RunInfo>(
        "SELECT protocol_id, start_date FROM protocol_runs
         WHERE id = $1 AND user_id = $2 AND status = 'active'",
    )
    .bind(run_id)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;

    let duration_days: i32 =
        sqlx::query_scalar("SELECT duration_days FROM protocols WHERE id = $1")
            .bind(run.protocol_id)
            .fetch_one(&mut *tx)
            .await?;

    let dose = log_dose_core(
        &mut tx,
        user_id,
        run.protocol_id,
        Some(run_id),
        run.start_date,
        duration_days,
        req,
    )
    .await?;

    tx.commit().await?;
    Ok(dose)
}

/// Delete a logged dose (undo): removes the `protocol_doses` row and, if it
/// created one, the linked `interventions` row, in a single transaction.
/// User-scoped through the dose's run. Returns `false` if the dose doesn't
/// exist or doesn't belong to a run owned by `user_id`.
pub async fn delete_dose(
    pool: &PgPool,
    user_id: Uuid,
    run_id: Uuid,
    dose_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;

    #[derive(sqlx::FromRow)]
    struct DoseInfo {
        intervention_id: Option<Uuid>,
    }

    let dose = sqlx::query_as::<_, DoseInfo>(
        "SELECT pd.intervention_id
         FROM protocol_doses pd
         JOIN protocol_runs r ON r.id = pd.run_id
         WHERE pd.id = $1 AND pd.run_id = $2 AND r.user_id = $3",
    )
    .bind(dose_id)
    .bind(run_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(dose) = dose else {
        return Ok(false);
    };

    sqlx::query("DELETE FROM protocol_doses WHERE id = $1")
        .bind(dose_id)
        .execute(&mut *tx)
        .await?;

    if let Some(intervention_id) = dose.intervention_id {
        sqlx::query("DELETE FROM interventions WHERE id = $1 AND user_id = $2")
            .bind(intervention_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    Ok(true)
}

/// Shared core of dose skipping, used by both `skip_dose_on_run` and the
/// legacy protocol-level `skip_dose` so run scoping and `skip_reason` bind
/// uniformly instead of drifting between the two route families.
async fn skip_dose_core(
    pool: &PgPool,
    protocol_id: Uuid,
    run_id: Option<Uuid>,
    req: &SkipDoseRequest,
) -> Result<ProtocolDoseRow, sqlx::Error> {
    // Verify line belongs to protocol
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM protocol_lines WHERE id = $1 AND protocol_id = $2",
    )
    .bind(req.protocol_line_id)
    .bind(protocol_id)
    .fetch_one(pool)
    .await?;

    sqlx::query_as::<_, ProtocolDoseRow>(
        "INSERT INTO protocol_doses (protocol_line_id, day_number, status, run_id, skip_reason)
         VALUES ($1, $2, 'skipped', $3, $4)
         RETURNING id, protocol_line_id, day_number, status, intervention_id, logged_at,
                   run_id, skip_reason",
    )
    .bind(req.protocol_line_id)
    .bind(req.day_number)
    .bind(run_id)
    .bind(&req.skip_reason)
    .fetch_one(pool)
    .await
}

/// Skip a dose on a run.
pub async fn skip_dose_on_run(
    pool: &PgPool,
    user_id: Uuid,
    run_id: Uuid,
    req: &SkipDoseRequest,
) -> Result<ProtocolDoseRow, sqlx::Error> {
    // Verify ownership
    let protocol_id: Uuid =
        sqlx::query_scalar("SELECT protocol_id FROM protocol_runs WHERE id = $1 AND user_id = $2")
            .bind(run_id)
            .bind(user_id)
            .fetch_one(pool)
            .await?;

    skip_dose_core(pool, protocol_id, Some(run_id), req).await
}

/// Legacy: Log a dose directly on a protocol (backward compatibility).
///
/// Writes are scoped to the protocol's *current* run — its active run, or
/// its most recently created run if none is active (the same choice
/// `get_by_id`/`get_shared` use) — via [`resolve_current_run_id`], so a dose
/// logged here shows up on the run-scoped dose grid instead of being
/// invisible on it or colliding with it on retry. Only a protocol with no
/// runs at all writes a `NULL` run_id.
pub async fn log_dose(
    pool: &PgPool,
    user_id: Uuid,
    protocol_id: Uuid,
    req: &LogDoseRequest,
    _config: &Config,
) -> Result<ProtocolDoseRow, DoseLogError> {
    let mut tx = pool.begin().await?;

    // Verify user owns the protocol
    let protocol = sqlx::query_as::<_, ProtocolRow>(
        "SELECT id, user_id, name, description, start_date, duration_days,
                status, is_template, tags, source_url,
                share_token, share_expires_at, created_at
         FROM protocols
         WHERE id = $1 AND user_id = $2",
    )
    .bind(protocol_id)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;

    let run_id = resolve_current_run_id(&mut *tx, protocol_id).await?;
    let start_date = match run_id {
        Some(run_id) => {
            sqlx::query_scalar("SELECT start_date FROM protocol_runs WHERE id = $1")
                .bind(run_id)
                .fetch_one(&mut *tx)
                .await?
        }
        None => protocol.start_date.ok_or(sqlx::Error::RowNotFound)?,
    };

    let dose = log_dose_core(
        &mut tx,
        user_id,
        protocol_id,
        run_id,
        start_date,
        protocol.duration_days,
        req,
    )
    .await?;

    tx.commit().await?;
    Ok(dose)
}

/// Legacy: Skip a dose directly on a protocol. Scoped to the protocol's
/// current run the same way [`log_dose`] is — see its doc comment.
pub async fn skip_dose(
    pool: &PgPool,
    user_id: Uuid,
    protocol_id: Uuid,
    req: &SkipDoseRequest,
) -> Result<ProtocolDoseRow, sqlx::Error> {
    // Verify ownership
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM protocols WHERE id = $1 AND user_id = $2")
        .bind(protocol_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

    let run_id = resolve_current_run_id(pool, protocol_id).await?;
    skip_dose_core(pool, protocol_id, run_id, req).await
}

/// Generate a share token with 7-day expiry.
pub async fn generate_share_token(
    pool: &PgPool,
    protocol_id: Uuid,
    user_id: Uuid,
) -> Result<(String, chrono::DateTime<Utc>), sqlx::Error> {
    let token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::days(7);

    let result = sqlx::query(
        "UPDATE protocols SET share_token = $3, share_expires_at = $4
         WHERE id = $1 AND user_id = $2",
    )
    .bind(protocol_id)
    .bind(user_id)
    .bind(&token)
    .bind(expires_at)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(sqlx::Error::RowNotFound);
    }

    Ok((token, expires_at))
}

/// Get a shared protocol (public, no user_id check, validates token not expired).
pub async fn get_shared(pool: &PgPool, token: &str) -> Result<ProtocolResponse, sqlx::Error> {
    let protocol = sqlx::query_as::<_, ProtocolRow>(
        "SELECT id, user_id, name, description, start_date, duration_days,
                status, is_template, tags, source_url,
                share_token, share_expires_at, created_at
         FROM protocols
         WHERE share_token = $1
           AND share_expires_at > NOW()",
    )
    .bind(token)
    .fetch_one(pool)
    .await?;

    // Same active-else-most-recent scoping as `get_by_id`, so a shared link
    // still shows the run's dose grid instead of an all-empty one.
    let current_run_id = resolve_current_run_id(pool, protocol.id).await?;
    let lines = fetch_lines_with_doses(pool, protocol.id, current_run_id).await?;

    Ok(build_response(protocol, lines, vec![]))
}

/// Import (copy) a shared protocol to a new user.
pub async fn import_protocol(
    pool: &PgPool,
    user_id: Uuid,
    token: &str,
) -> Result<ProtocolRow, sqlx::Error> {
    // Fetch the shared protocol
    let source = sqlx::query_as::<_, ProtocolRow>(
        "SELECT id, user_id, name, description, start_date, duration_days,
                status, is_template, tags, source_url,
                share_token, share_expires_at, created_at
         FROM protocols
         WHERE share_token = $1
           AND share_expires_at > NOW()",
    )
    .bind(token)
    .fetch_one(pool)
    .await?;

    let source_lines = sqlx::query_as::<_, ProtocolLineRow>(
        "SELECT id, protocol_id, substance, dose, unit, route, time_of_day,
                schedule_pattern, sort_order, created_at
         FROM protocol_lines
         WHERE protocol_id = $1
         ORDER BY sort_order",
    )
    .bind(source.id)
    .fetch_all(pool)
    .await?;

    // Copy to new user in a transaction — as a recipe (no start_date)
    let mut tx = pool.begin().await?;

    let new_protocol = sqlx::query_as::<_, ProtocolRow>(
        "INSERT INTO protocols (user_id, name, description, duration_days)
         VALUES ($1, $2, $3, $4)
         RETURNING id, user_id, name, description, start_date, duration_days,
                   status, is_template, tags, source_url,
                   share_token, share_expires_at, created_at",
    )
    .bind(user_id)
    .bind(&source.name)
    .bind(&source.description)
    .bind(source.duration_days)
    .fetch_one(&mut *tx)
    .await?;

    for line in &source_lines {
        sqlx::query(
            "INSERT INTO protocol_lines
                (protocol_id, substance, dose, unit, route, time_of_day, schedule_pattern, sort_order)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(new_protocol.id)
        .bind(&line.substance)
        .bind(line.dose)
        .bind(&line.unit)
        .bind(&line.route)
        .bind(&line.time_of_day)
        .bind(&line.schedule_pattern)
        .bind(line.sort_order)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(new_protocol)
}

/// Get today's doses across all active runs for a user.
pub async fn todays_doses(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<TodaysDoseItem>, sqlx::Error> {
    sqlx::query_as::<_, TodaysDoseItem>(
        "SELECT
            p.id AS protocol_id,
            p.name AS protocol_name,
            r.id AS run_id,
            pl.id AS protocol_line_id,
            pl.substance,
            pl.dose,
            pl.unit,
            pl.route,
            pl.time_of_day,
            (CURRENT_DATE - r.start_date) AS day_number,
            pd.status
         FROM protocol_runs r
         JOIN protocols p ON p.id = r.protocol_id
         JOIN protocol_lines pl ON pl.protocol_id = p.id
         LEFT JOIN protocol_doses pd
             ON pd.protocol_line_id = pl.id
             AND pd.run_id = r.id
             AND pd.day_number = (CURRENT_DATE - r.start_date)
         WHERE r.user_id = $1
           AND r.status = 'active'
           AND (CURRENT_DATE - r.start_date) >= 0
           AND (CURRENT_DATE - r.start_date) < p.duration_days
           AND (pl.schedule_pattern->((CURRENT_DATE - r.start_date)::int))::text = 'true'
         ORDER BY pl.sort_order",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Get distinct active substances from active runs (for intervention quick-pick).
pub async fn active_substances(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<ActiveSubstanceItem>, sqlx::Error> {
    sqlx::query_as::<_, ActiveSubstanceItem>(
        "SELECT DISTINCT ON (pl.substance, pl.dose, pl.unit, pl.route)
            pl.substance,
            pl.dose,
            pl.unit,
            pl.route,
            p.name AS protocol_name
         FROM protocol_runs r
         JOIN protocols p ON p.id = r.protocol_id
         JOIN protocol_lines pl ON pl.protocol_id = p.id
         WHERE r.user_id = $1
           AND r.status = 'active'
         ORDER BY pl.substance, pl.dose, pl.unit, pl.route, p.name",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

// --- Adherence / dose-status ---

/// Errors from `run_doses` that need a distinct HTTP status.
#[derive(Debug, thiserror::Error)]
pub enum RunDosesError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    /// Out-of-bounds or backwards `from_day`/`to_day` — maps to 400.
    #[error("{0}")]
    Invalid(String),
}

#[derive(sqlx::FromRow)]
struct RunInfo {
    protocol_id: Uuid,
    start_date: NaiveDate,
    duration_days: i32,
    /// `CURRENT_DATE` from Postgres, fetched in the same query as the run
    /// row. Adherence math has one clock — the database's — never the
    /// application server's local clock.
    today: NaiveDate,
}

async fn fetch_run_info(
    pool: &PgPool,
    run_id: Uuid,
    user_id: Uuid,
) -> Result<Option<RunInfo>, sqlx::Error> {
    sqlx::query_as::<_, RunInfo>(
        "SELECT r.protocol_id, r.start_date, p.duration_days, CURRENT_DATE AS today
         FROM protocol_runs r
         JOIN protocols p ON p.id = r.protocol_id
         WHERE r.id = $1 AND r.user_id = $2",
    )
    .bind(run_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// List the dose status of every scheduled (line, day) pair in
/// `[from_day, to_day]` for a run. Days that aren't scheduled (pattern
/// false, or out of the run's duration) are omitted — see
/// [`crate::dose_status::compute_dose_status`].
///
/// Defaults: `from_day = 0`, `to_day = min(today_day, duration_days - 1)`
/// (today_day from Postgres `CURRENT_DATE`, not the app server's clock).
/// If the caller doesn't pass either bound and the run hasn't started yet
/// (nothing scheduled so far), this returns an empty list rather than an
/// error. Explicit out-of-bounds or backwards bounds are a 400.
pub async fn run_doses(
    pool: &PgPool,
    user_id: Uuid,
    run_id: Uuid,
    from_day: Option<i32>,
    to_day: Option<i32>,
) -> Result<Vec<RunDoseItem>, RunDosesError> {
    let run = fetch_run_info(pool, run_id, user_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;

    let today_day = dose_status::today_day(run.start_date, run.today);
    let explicit = from_day.is_some() || to_day.is_some();
    let from = from_day.unwrap_or(0);
    let to = to_day.unwrap_or(today_day.min(run.duration_days - 1));

    if from < 0 || to >= run.duration_days {
        return Err(RunDosesError::Invalid(format!(
            "from_day/to_day must be within [0, {}); got from_day={from}, to_day={to}",
            run.duration_days
        )));
    }
    if from > to {
        if explicit {
            return Err(RunDosesError::Invalid(format!(
                "from_day ({from}) must be <= to_day ({to})"
            )));
        }
        // Default range on a run that hasn't started yet — nothing
        // scheduled so far, not an error.
        return Ok(vec![]);
    }

    let lines = sqlx::query_as::<_, ProtocolLineRow>(
        "SELECT id, protocol_id, substance, dose, unit, route, time_of_day,
                schedule_pattern, sort_order, created_at
         FROM protocol_lines
         WHERE protocol_id = $1
         ORDER BY sort_order",
    )
    .bind(run.protocol_id)
    .fetch_all(pool)
    .await?;

    let doses = sqlx::query_as::<_, ProtocolDoseRow>(
        "SELECT id, protocol_line_id, day_number, status, intervention_id, logged_at,
                run_id, skip_reason
         FROM protocol_doses
         WHERE run_id = $1 AND day_number BETWEEN $2 AND $3",
    )
    .bind(run_id)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;

    let mut by_line_day: std::collections::HashMap<(Uuid, i32), &ProtocolDoseRow> =
        std::collections::HashMap::new();
    for d in &doses {
        by_line_day.insert((d.protocol_line_id, d.day_number), d);
    }

    let mut result = Vec::new();
    for day in from..=to {
        for line in &lines {
            let pattern: Vec<bool> = line
                .schedule_pattern
                .as_array()
                .map(|a| a.iter().map(|v| v.as_bool().unwrap_or(false)).collect())
                .unwrap_or_default();
            let existing = by_line_day.get(&(line.id, day));
            let status = dose_status::compute_dose_status(
                day,
                run.duration_days,
                &pattern,
                existing.map(|d| d.status.as_str()),
                today_day,
            );
            let Some(status) = status else { continue };

            result.push(RunDoseItem {
                day_number: day,
                date: run.start_date + Duration::days(i64::from(day)),
                protocol_line_id: line.id,
                substance: line.substance.clone(),
                dose: line.dose,
                unit: line.unit.clone(),
                route: line.route.clone(),
                time_of_day: line.time_of_day.clone(),
                status: status.as_str().to_string(),
                dose_id: existing.map(|d| d.id),
                intervention_id: existing.and_then(|d| d.intervention_id),
                skip_reason: existing.and_then(|d| d.skip_reason.clone()),
                logged_at: existing.map(|d| d.logged_at),
            });
        }
    }

    Ok(result)
}

/// Scheduled days, in the past, with no dose row, across *all* of the
/// user's active runs. Capped at 200 rows (most recent first) — see the
/// `missed-doses` route doc comment.
pub async fn missed_doses(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<MissedDoseItem>, sqlx::Error> {
    sqlx::query_as::<_, MissedDoseItem>(
        "SELECT
            p.id AS protocol_id,
            p.name AS protocol_name,
            r.id AS run_id,
            pl.id AS protocol_line_id,
            pl.substance,
            pl.dose,
            pl.unit,
            pl.route,
            pl.time_of_day,
            gs.day_number,
            (r.start_date + gs.day_number) AS date,
            'missed'::text AS status
         FROM protocol_runs r
         JOIN protocols p ON p.id = r.protocol_id
         JOIN protocol_lines pl ON pl.protocol_id = p.id
         CROSS JOIN LATERAL generate_series(
             0, LEAST((CURRENT_DATE - r.start_date)::int - 1, p.duration_days - 1)
         ) AS gs(day_number)
         WHERE r.user_id = $1
           AND r.status = 'active'
           AND (pl.schedule_pattern->gs.day_number)::text = 'true'
           AND NOT EXISTS (
               SELECT 1 FROM protocol_doses pd
               WHERE pd.protocol_line_id = pl.id
                 AND pd.run_id = r.id
                 AND pd.day_number = gs.day_number
           )
         ORDER BY date DESC
         LIMIT 200",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Per-line adherence aggregate rows for one run, bounded to
/// `[0, sched_bound]` for scheduled/completed/skipped and `[0, missed_bound]`
/// for missed (negative bounds yield an empty `generate_series`, i.e. all
/// zero counts — used for runs that haven't started yet).
async fn fetch_line_adherence(
    pool: &PgPool,
    protocol_id: Uuid,
    run_id: Uuid,
    sched_bound: i32,
    missed_bound: i32,
) -> Result<Vec<LineAdherenceRow>, sqlx::Error> {
    sqlx::query_as::<_, LineAdherenceRow>(
        "SELECT
            pl.id AS protocol_line_id,
            pl.substance,
            COALESCE((
                SELECT COUNT(*) FROM generate_series(0, $2) AS gs(day_number)
                WHERE (pl.schedule_pattern->gs.day_number)::text = 'true'
            ), 0) AS scheduled_so_far,
            COALESCE((
                SELECT COUNT(*) FROM protocol_doses pd
                WHERE pd.protocol_line_id = pl.id
                  AND pd.run_id = $3
                  AND pd.status = 'completed'
                  AND pd.day_number <= $2
            ), 0) AS completed,
            COALESCE((
                SELECT COUNT(*) FROM protocol_doses pd
                WHERE pd.protocol_line_id = pl.id
                  AND pd.run_id = $3
                  AND pd.status = 'skipped'
                  AND pd.day_number <= $2
            ), 0) AS skipped,
            COALESCE((
                SELECT COUNT(*) FROM generate_series(0, $4) AS gs(day_number)
                WHERE (pl.schedule_pattern->gs.day_number)::text = 'true'
                  AND NOT EXISTS (
                      SELECT 1 FROM protocol_doses pd
                      WHERE pd.protocol_line_id = pl.id
                        AND pd.run_id = $3
                        AND pd.day_number = gs.day_number
                  )
            ), 0) AS missed
         FROM protocol_lines pl
         WHERE pl.protocol_id = $1
         ORDER BY pl.sort_order",
    )
    .bind(protocol_id)
    .bind(sched_bound)
    .bind(run_id)
    .bind(missed_bound)
    .fetch_all(pool)
    .await
}

/// Sum per-line adherence rows into run-level totals + adherence_pct.
fn summarize_adherence(lines: &[LineAdherenceRow]) -> (i64, i64, i64, i64, Option<f64>) {
    let scheduled_so_far: i64 = lines.iter().map(|l| l.scheduled_so_far).sum();
    let completed: i64 = lines.iter().map(|l| l.completed).sum();
    let skipped: i64 = lines.iter().map(|l| l.skipped).sum();
    let missed: i64 = lines.iter().map(|l| l.missed).sum();
    let pct = if scheduled_so_far == 0 {
        None
    } else {
        Some(completed as f64 / scheduled_so_far as f64 * 100.0)
    };
    (scheduled_so_far, completed, skipped, missed, pct)
}

/// `(adherence_pct, doses_missed)` for a single run — used to populate
/// `RunResponse` right after `create_run`, without a second round-trip
/// through the ownership check (the caller already knows `protocol_id`).
pub async fn run_adherence_totals(
    pool: &PgPool,
    protocol_id: Uuid,
    run_id: Uuid,
    start_date: NaiveDate,
    duration_days: i32,
) -> Result<(Option<f64>, i64), sqlx::Error> {
    let today: NaiveDate = sqlx::query_scalar("SELECT CURRENT_DATE")
        .fetch_one(pool)
        .await?;
    let today_day = dose_status::today_day(start_date, today);
    let sched_bound = today_day.min(duration_days - 1);
    let missed_bound = (today_day - 1).min(duration_days - 1);

    let lines = fetch_line_adherence(pool, protocol_id, run_id, sched_bound, missed_bound).await?;
    let (_, _, _, missed, pct) = summarize_adherence(&lines);
    Ok((pct, missed))
}

/// Full per-line + run-level adherence breakdown for `GET
/// /protocols/runs/:run_id/adherence`. `scheduled_so_far` includes today
/// (a dose logged today counts); `missed` only counts days strictly before
/// today (today itself is "pending", not "missed").
pub async fn run_adherence(
    pool: &PgPool,
    user_id: Uuid,
    run_id: Uuid,
) -> Result<AdherenceResponse, sqlx::Error> {
    let run = fetch_run_info(pool, run_id, user_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;

    let today_day = dose_status::today_day(run.start_date, run.today);
    let sched_bound = today_day.min(run.duration_days - 1);
    let missed_bound = (today_day - 1).min(run.duration_days - 1);

    let lines =
        fetch_line_adherence(pool, run.protocol_id, run_id, sched_bound, missed_bound).await?;

    let (scheduled_so_far, completed, skipped, missed, adherence_pct) = summarize_adherence(&lines);

    let line_responses = lines
        .into_iter()
        .map(|l| {
            let pct = if l.scheduled_so_far == 0 {
                None
            } else {
                Some(l.completed as f64 / l.scheduled_so_far as f64 * 100.0)
            };
            LineAdherence {
                protocol_line_id: l.protocol_line_id,
                substance: l.substance,
                scheduled_so_far: l.scheduled_so_far,
                completed: l.completed,
                skipped: l.skipped,
                missed: l.missed,
                adherence_pct: pct,
            }
        })
        .collect();

    Ok(AdherenceResponse {
        run_id,
        scheduled_so_far,
        completed,
        skipped,
        missed,
        adherence_pct,
        lines: line_responses,
    })
}

// --- Notification Preferences ---

/// Get or create notification preferences for a user.
pub async fn get_notification_preferences(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<NotificationPreferencesRow, sqlx::Error> {
    // Try inserting defaults; if conflict, the row already exists.
    sqlx::query(
        "INSERT INTO user_notification_preferences (user_id)
         VALUES ($1)
         ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, NotificationPreferencesRow>(
        "SELECT user_id, default_notify, default_notify_times,
                repeat_reminders, repeat_interval_minutes, updated_at
         FROM user_notification_preferences
         WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Update notification preferences for a user.
pub async fn update_notification_preferences(
    pool: &PgPool,
    user_id: Uuid,
    req: &UpdateNotificationPreferences,
) -> Result<NotificationPreferencesRow, sqlx::Error> {
    let notify_times = req
        .default_notify_times
        .as_ref()
        .map(|t| serde_json::to_value(t).unwrap_or_default());

    sqlx::query_as::<_, NotificationPreferencesRow>(
        "INSERT INTO user_notification_preferences (user_id, default_notify, default_notify_times,
                                                     repeat_reminders, repeat_interval_minutes)
         VALUES ($1, COALESCE($2, false), COALESCE($3, '[\"08:00\"]'::jsonb),
                 COALESCE($4, false), COALESCE($5, 30))
         ON CONFLICT (user_id) DO UPDATE SET
             default_notify = COALESCE($2, user_notification_preferences.default_notify),
             default_notify_times = COALESCE($3, user_notification_preferences.default_notify_times),
             repeat_reminders = COALESCE($4, user_notification_preferences.repeat_reminders),
             repeat_interval_minutes = COALESCE($5, user_notification_preferences.repeat_interval_minutes),
             updated_at = now()
         RETURNING user_id, default_notify, default_notify_times,
                   repeat_reminders, repeat_interval_minutes, updated_at",
    )
    .bind(user_id)
    .bind(req.default_notify)
    .bind(&notify_times)
    .bind(req.repeat_reminders)
    .bind(req.repeat_interval_minutes)
    .fetch_one(pool)
    .await
}

// --- Push Tokens ---

/// Register a push token for a user. Upserts on (user_id, device_token).
pub async fn register_push_token(
    pool: &PgPool,
    user_id: Uuid,
    req: &RegisterPushTokenRequest,
) -> Result<PushTokenRow, sqlx::Error> {
    sqlx::query_as::<_, PushTokenRow>(
        "INSERT INTO push_tokens (user_id, device_token, platform)
         VALUES ($1, $2, $3)
         ON CONFLICT (user_id, device_token) DO UPDATE SET
             platform = $3,
             created_at = now()
         RETURNING id, user_id, device_token, platform, created_at",
    )
    .bind(user_id)
    .bind(&req.device_token)
    .bind(&req.platform)
    .fetch_one(pool)
    .await
}

/// Delete a push token.
pub async fn delete_push_token(
    pool: &PgPool,
    user_id: Uuid,
    device_token: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM push_tokens WHERE user_id = $1 AND device_token = $2")
        .bind(user_id)
        .bind(device_token)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// --- Template & Export/Import functions ---

/// List all protocol templates, ordered by name.
pub async fn list_templates(pool: &PgPool) -> Result<Vec<TemplateListItem>, sqlx::Error> {
    sqlx::query_as::<_, TemplateListItem>(
        "SELECT id, name, description, duration_days, tags, created_at
         FROM protocols
         WHERE is_template = true
         ORDER BY name",
    )
    .fetch_all(pool)
    .await
}

/// Export a protocol to the portable JSON format.
pub async fn export_protocol(
    pool: &PgPool,
    protocol_id: Uuid,
    user_id: Uuid,
) -> Result<ProtocolExport, sqlx::Error> {
    let protocol = sqlx::query_as::<_, ProtocolRow>(
        "SELECT id, user_id, name, description, start_date, duration_days,
                status, is_template, tags, source_url,
                share_token, share_expires_at, created_at
         FROM protocols
         WHERE id = $1 AND user_id = $2",
    )
    .bind(protocol_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let lines = sqlx::query_as::<_, ProtocolLineRow>(
        "SELECT id, protocol_id, substance, dose, unit, route, time_of_day,
                schedule_pattern, sort_order, created_at
         FROM protocol_lines
         WHERE protocol_id = $1
         ORDER BY sort_order",
    )
    .bind(protocol_id)
    .fetch_all(pool)
    .await?;

    let tags = protocol
        .tags
        .as_ref()
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Ok(ProtocolExport {
        schema: "ownpulse-protocol/v1".to_string(),
        name: protocol.name,
        description: protocol.description,
        tags,
        duration_days: protocol.duration_days,
        lines: lines
            .into_iter()
            .map(|l| ProtocolLineExport {
                substance: l.substance,
                dose: l.dose,
                unit: l.unit,
                route: l.route,
                time_of_day: l.time_of_day,
                pattern: l.schedule_pattern,
            })
            .collect(),
    })
}

/// Import a protocol from the portable export format for a user.
pub async fn import_protocol_from_export(
    pool: &PgPool,
    user_id: Uuid,
    start_date: NaiveDate,
    export: &ProtocolExport,
) -> Result<ProtocolRow, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let tags_json = serde_json::to_value(&export.tags).unwrap_or_default();

    let protocol = sqlx::query_as::<_, ProtocolRow>(
        "INSERT INTO protocols (user_id, name, description, start_date, duration_days, tags)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, user_id, name, description, start_date, duration_days,
                   status, is_template, tags, source_url,
                   share_token, share_expires_at, created_at",
    )
    .bind(user_id)
    .bind(&export.name)
    .bind(&export.description)
    .bind(start_date)
    .bind(export.duration_days)
    .bind(&tags_json)
    .fetch_one(&mut *tx)
    .await?;

    for (i, line) in export.lines.iter().enumerate() {
        let pattern = expand_pattern(&line.pattern, export.duration_days);
        let pattern_json = serde_json::to_value(&pattern).unwrap_or_default();

        sqlx::query(
            "INSERT INTO protocol_lines
                (protocol_id, substance, dose, unit, route, time_of_day, schedule_pattern, sort_order)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(protocol.id)
        .bind(&line.substance)
        .bind(line.dose)
        .bind(&line.unit)
        .bind(&line.route)
        .bind(&line.time_of_day)
        .bind(&pattern_json)
        .bind(i as i32)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(protocol)
}

/// Admin: promote a protocol to a template (set is_template=true, tags, user_id=NULL).
pub async fn promote_to_template(
    pool: &PgPool,
    protocol_id: Uuid,
    tags: Option<Vec<String>>,
) -> Result<bool, sqlx::Error> {
    let tags_json = tags
        .map(|t| serde_json::to_value(t).unwrap_or_default())
        .unwrap_or_else(|| serde_json::Value::Array(vec![]));

    let result = sqlx::query(
        "UPDATE protocols
         SET is_template = true, tags = $2, user_id = NULL
         WHERE id = $1",
    )
    .bind(protocol_id)
    .bind(&tags_json)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Admin: demote a template back to a regular protocol.
pub async fn demote_template(pool: &PgPool, protocol_id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE protocols SET is_template = false WHERE id = $1 AND is_template = true",
    )
    .bind(protocol_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Admin: bulk import templates. Upserts by name + source_url. Returns count imported.
pub async fn bulk_import_templates(
    pool: &PgPool,
    exports: &[ProtocolExport],
    source_url: Option<&str>,
) -> Result<usize, sqlx::Error> {
    let mut count = 0usize;
    let today = Utc::now().date_naive();

    for export in exports {
        let mut tx = pool.begin().await?;

        let tags_json = serde_json::to_value(&export.tags).unwrap_or_default();

        // Check for existing template with same name and source_url
        let existing: Option<Uuid> = if let Some(url) = source_url {
            sqlx::query_scalar(
                "SELECT id FROM protocols
                 WHERE is_template = true AND name = $1 AND source_url = $2",
            )
            .bind(&export.name)
            .bind(url)
            .fetch_optional(&mut *tx)
            .await?
        } else {
            None
        };

        let protocol_id = if let Some(existing_id) = existing {
            // Update existing template
            sqlx::query(
                "UPDATE protocols
                 SET description = $2, duration_days = $3, tags = $4
                 WHERE id = $1",
            )
            .bind(existing_id)
            .bind(&export.description)
            .bind(export.duration_days)
            .bind(&tags_json)
            .execute(&mut *tx)
            .await?;

            // Delete old lines to replace
            sqlx::query("DELETE FROM protocol_lines WHERE protocol_id = $1")
                .bind(existing_id)
                .execute(&mut *tx)
                .await?;

            existing_id
        } else {
            // Insert new template
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO protocols
                    (user_id, name, description, start_date, duration_days,
                     is_template, tags, source_url)
                 VALUES (NULL, $1, $2, $3, $4, true, $5, $6)
                 RETURNING id",
            )
            .bind(&export.name)
            .bind(&export.description)
            .bind(today)
            .bind(export.duration_days)
            .bind(&tags_json)
            .bind(source_url)
            .fetch_one(&mut *tx)
            .await?
        };

        // Insert lines
        for (i, line) in export.lines.iter().enumerate() {
            let pattern = expand_pattern(&line.pattern, export.duration_days);
            let pattern_json = serde_json::to_value(&pattern).unwrap_or_default();

            sqlx::query(
                "INSERT INTO protocol_lines
                    (protocol_id, substance, dose, unit, route, time_of_day, schedule_pattern, sort_order)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(protocol_id)
            .bind(&line.substance)
            .bind(line.dose)
            .bind(&line.unit)
            .bind(&line.route)
            .bind(&line.time_of_day)
            .bind(&pattern_json)
            .bind(i as i32)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        count += 1;
    }

    Ok(count)
}

/// Copy a template to a user with an optional start date (recipe by default).
pub async fn copy_template(
    pool: &PgPool,
    template_id: Uuid,
    user_id: Uuid,
    start_date: Option<NaiveDate>,
) -> Result<ProtocolRow, sqlx::Error> {
    // Verify it's a template
    let template = sqlx::query_as::<_, ProtocolRow>(
        "SELECT id, user_id, name, description, start_date, duration_days,
                status, is_template, tags, source_url,
                share_token, share_expires_at, created_at
         FROM protocols
         WHERE id = $1 AND is_template = true",
    )
    .bind(template_id)
    .fetch_one(pool)
    .await?;

    let source_lines = sqlx::query_as::<_, ProtocolLineRow>(
        "SELECT id, protocol_id, substance, dose, unit, route, time_of_day,
                schedule_pattern, sort_order, created_at
         FROM protocol_lines
         WHERE protocol_id = $1
         ORDER BY sort_order",
    )
    .bind(template_id)
    .fetch_all(pool)
    .await?;

    let mut tx = pool.begin().await?;

    let new_protocol = sqlx::query_as::<_, ProtocolRow>(
        "INSERT INTO protocols (user_id, name, description, start_date, duration_days)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, user_id, name, description, start_date, duration_days,
                   status, is_template, tags, source_url,
                   share_token, share_expires_at, created_at",
    )
    .bind(user_id)
    .bind(&template.name)
    .bind(&template.description)
    .bind(start_date)
    .bind(template.duration_days)
    .fetch_one(&mut *tx)
    .await?;

    for line in &source_lines {
        sqlx::query(
            "INSERT INTO protocol_lines
                (protocol_id, substance, dose, unit, route, time_of_day, schedule_pattern, sort_order)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(new_protocol.id)
        .bind(&line.substance)
        .bind(line.dose)
        .bind(&line.unit)
        .bind(&line.route)
        .bind(&line.time_of_day)
        .bind(&line.schedule_pattern)
        .bind(line.sort_order)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(new_protocol)
}

/// Expand a pattern shorthand or pass through a bool array.
fn expand_pattern(pattern: &serde_json::Value, duration_days: i32) -> Vec<bool> {
    match pattern.as_str() {
        Some("daily") => vec![true; duration_days as usize],
        Some("mwf") => (0..duration_days as usize)
            .map(|d| matches!(d % 7, 0 | 2 | 4))
            .collect(),
        Some("eod") => (0..duration_days as usize).map(|d| d % 2 == 0).collect(),
        Some("weekdays") => (0..duration_days as usize).map(|d| d % 7 < 5).collect(),
        _ => pattern
            .as_array()
            .map(|a| a.iter().map(|v| v.as_bool().unwrap_or(false)).collect())
            .unwrap_or_default(),
    }
}

// --- Helpers ---

/// Fetch a protocol's lines together with their logged doses.
///
/// `run_id` scopes which run's doses are attached to each line: pass the id
/// of the run currently being viewed (e.g. the active run), or `None` to see
/// only legacy protocol-level doses (`run_id IS NULL`, from before runs
/// existed / the deprecated `/protocols/:id/doses/*` endpoints). Without this
/// scoping, a second run of the same protocol would show the first run's
/// checkmarks on its dose grid.
async fn fetch_lines_with_doses(
    pool: &PgPool,
    protocol_id: Uuid,
    run_id: Option<Uuid>,
) -> Result<Vec<ProtocolLineResponse>, sqlx::Error> {
    let lines = sqlx::query_as::<_, ProtocolLineRow>(
        "SELECT id, protocol_id, substance, dose, unit, route, time_of_day,
                schedule_pattern, sort_order, created_at
         FROM protocol_lines
         WHERE protocol_id = $1
         ORDER BY sort_order",
    )
    .bind(protocol_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::with_capacity(lines.len());
    for line in lines {
        let doses = sqlx::query_as::<_, ProtocolDoseRow>(
            "SELECT id, protocol_line_id, day_number, status, intervention_id, logged_at,
                    run_id, skip_reason
             FROM protocol_doses
             WHERE protocol_line_id = $1 AND run_id IS NOT DISTINCT FROM $2
             ORDER BY day_number",
        )
        .bind(line.id)
        .bind(run_id)
        .fetch_all(pool)
        .await?;

        result.push(ProtocolLineResponse {
            id: line.id,
            protocol_id: line.protocol_id,
            substance: line.substance,
            dose: line.dose,
            unit: line.unit,
            route: line.route,
            time_of_day: line.time_of_day,
            schedule_pattern: line.schedule_pattern,
            sort_order: line.sort_order,
            created_at: line.created_at,
            doses,
        });
    }

    Ok(result)
}

fn run_to_response(
    run: ProtocolRunRow,
    protocol_name: Option<String>,
    duration_days: Option<i32>,
) -> RunResponse {
    let today = Utc::now().date_naive();
    let progress_pct = if let Some(dur) = duration_days {
        if run.status == "completed" {
            100.0
        } else if today < run.start_date {
            0.0
        } else {
            let elapsed = (today - run.start_date).num_days() as f64;
            (elapsed / dur as f64 * 100.0).min(100.0)
        }
    } else {
        0.0
    };

    RunResponse {
        id: run.id,
        protocol_id: run.protocol_id,
        protocol_name,
        user_id: run.user_id,
        start_date: run.start_date,
        duration_days,
        status: run.status,
        notify: run.notify,
        notify_time: run.notify_time,
        notify_times: run.notify_times,
        repeat_reminders: run.repeat_reminders,
        repeat_interval_minutes: run.repeat_interval_minutes,
        progress_pct,
        doses_today: 0,
        doses_completed_today: 0,
        // Not computed here — see the doc comment on `RunResponse`. Only
        // `list_active_runs` and `create_run` populate real values.
        adherence_pct: None,
        doses_missed: 0,
        created_at: run.created_at,
    }
}

fn build_response(
    protocol: ProtocolRow,
    lines: Vec<ProtocolLineResponse>,
    runs: Vec<RunResponse>,
) -> ProtocolResponse {
    let tags = protocol
        .tags
        .as_ref()
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    ProtocolResponse {
        id: protocol.id,
        user_id: protocol.user_id,
        name: protocol.name,
        description: protocol.description,
        start_date: protocol.start_date,
        duration_days: protocol.duration_days,
        status: protocol.status,
        is_template: protocol.is_template,
        tags,
        share_token: protocol.share_token,
        share_expires_at: protocol.share_expires_at,
        created_at: protocol.created_at,
        lines,
        runs,
    }
}
