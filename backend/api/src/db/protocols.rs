// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{Duration, NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::Config;
use crate::models::protocol::{
    CreateProtocol, LogDoseRequest, ProtocolDoseRow, ProtocolExport, ProtocolLineExport,
    ProtocolLineResponse, ProtocolLineRow, ProtocolListItem, ProtocolResponse, ProtocolRow,
    SkipDoseRequest, TemplateListItem, TodaysDoseItem, UpdateProtocol,
};

/// Insert a new protocol with its lines in a transaction.
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

/// List protocols for a user with computed progress.
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
            CASE
                WHEN p.status = 'completed' THEN 100.0
                WHEN CURRENT_DATE < p.start_date THEN 0.0
                ELSE LEAST(
                    100.0,
                    (CURRENT_DATE - p.start_date)::double precision / p.duration_days * 100.0
                )
            END AS progress_pct,
            (
                SELECT pl.substance
                FROM protocol_lines pl
                LEFT JOIN protocol_doses pd
                    ON pd.protocol_line_id = pl.id
                    AND pd.day_number = (CURRENT_DATE - p.start_date)
                WHERE pl.protocol_id = p.id
                    AND pd.id IS NULL
                    AND (CURRENT_DATE - p.start_date) >= 0
                    AND (CURRENT_DATE - p.start_date) < p.duration_days
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

/// Get a full protocol with lines and doses.
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

    let lines = fetch_lines_with_doses(pool, protocol_id).await?;

    Ok(build_response(protocol, lines))
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

/// Log a dose: verify ownership, validate schedule, create intervention, insert dose.
pub async fn log_dose(
    pool: &PgPool,
    user_id: Uuid,
    protocol_id: Uuid,
    req: &LogDoseRequest,
    _config: &Config,
) -> Result<ProtocolDoseRow, sqlx::Error> {
    let mut tx = pool.begin().await?;

    // 1. Verify user owns the protocol
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

    // 2. Get the protocol_line (verify it belongs to the protocol)
    let line = sqlx::query_as::<_, ProtocolLineRow>(
        "SELECT id, protocol_id, substance, dose, unit, route, time_of_day,
                schedule_pattern, sort_order, created_at
         FROM protocol_lines
         WHERE id = $1 AND protocol_id = $2",
    )
    .bind(req.line_id)
    .bind(protocol_id)
    .fetch_one(&mut *tx)
    .await?;

    // 3. Verify the day_number is valid and schedule_pattern[day_number] is true
    let pattern = line
        .schedule_pattern
        .as_array()
        .ok_or(sqlx::Error::RowNotFound)?;

    if req.day_number < 0 || req.day_number >= protocol.duration_days {
        return Err(sqlx::Error::RowNotFound);
    }

    let scheduled = pattern
        .get(req.day_number as usize)
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !scheduled {
        return Err(sqlx::Error::RowNotFound);
    }

    // 4. Create an intervention record
    let administered_at = protocol.start_date + chrono::Duration::days(i64::from(req.day_number));
    let administered_at_utc = administered_at
        .and_hms_opt(12, 0, 0)
        .unwrap_or_default()
        .and_utc();

    let intervention_id: Uuid = sqlx::query_scalar(
        "INSERT INTO interventions
            (user_id, substance, dose, unit, route, administered_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(user_id)
    .bind(&line.substance)
    .bind(line.dose)
    .bind(&line.unit)
    .bind(&line.route)
    .bind(administered_at_utc)
    .fetch_one(&mut *tx)
    .await?;

    // 5. Insert protocol_dose
    let dose = sqlx::query_as::<_, ProtocolDoseRow>(
        "INSERT INTO protocol_doses (protocol_line_id, day_number, status, intervention_id)
         VALUES ($1, $2, 'completed', $3)
         RETURNING id, protocol_line_id, day_number, status, intervention_id, logged_at",
    )
    .bind(req.line_id)
    .bind(req.day_number)
    .bind(intervention_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(dose)
}

/// Skip a dose.
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

    // Verify line belongs to protocol
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM protocol_lines WHERE id = $1 AND protocol_id = $2",
    )
    .bind(req.line_id)
    .bind(protocol_id)
    .fetch_one(pool)
    .await?;

    sqlx::query_as::<_, ProtocolDoseRow>(
        "INSERT INTO protocol_doses (protocol_line_id, day_number, status)
         VALUES ($1, $2, 'skipped')
         RETURNING id, protocol_line_id, day_number, status, intervention_id, logged_at",
    )
    .bind(req.line_id)
    .bind(req.day_number)
    .fetch_one(pool)
    .await
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

    let lines = fetch_lines_with_doses(pool, protocol.id).await?;

    Ok(build_response(protocol, lines))
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

    // Copy to new user in a transaction
    let mut tx = pool.begin().await?;

    let today = Utc::now().date_naive();

    let new_protocol = sqlx::query_as::<_, ProtocolRow>(
        "INSERT INTO protocols (user_id, name, description, start_date, duration_days)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, user_id, name, description, start_date, duration_days,
                   status, is_template, tags, source_url,
                   share_token, share_expires_at, created_at",
    )
    .bind(user_id)
    .bind(&source.name)
    .bind(&source.description)
    .bind(today)
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

/// Get today's doses across all active protocols for a user.
pub async fn todays_doses(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<TodaysDoseItem>, sqlx::Error> {
    // We calculate day_number = CURRENT_DATE - start_date for each active protocol,
    // then check which lines are scheduled for that day.
    // Using a raw query with JSONB array access.
    sqlx::query_as::<_, TodaysDoseItem>(
        "SELECT
            p.id AS protocol_id,
            p.name AS protocol_name,
            pl.id AS line_id,
            pl.substance,
            pl.dose,
            pl.unit,
            pl.route,
            pl.time_of_day,
            (CURRENT_DATE - p.start_date) AS day_number,
            pd.status
         FROM protocols p
         JOIN protocol_lines pl ON pl.protocol_id = p.id
         LEFT JOIN protocol_doses pd
             ON pd.protocol_line_id = pl.id
             AND pd.day_number = (CURRENT_DATE - p.start_date)
         WHERE p.user_id = $1
           AND p.status = 'active'
           AND (CURRENT_DATE - p.start_date) >= 0
           AND (CURRENT_DATE - p.start_date) < p.duration_days
           AND (pl.schedule_pattern->((CURRENT_DATE - p.start_date)::int)::text)::text = 'true'
         ORDER BY pl.sort_order",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
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

/// Get a protocol by id (no user_id check — for admin/template operations).
pub async fn get_by_id_unscoped(
    pool: &PgPool,
    protocol_id: Uuid,
) -> Result<ProtocolResponse, sqlx::Error> {
    let protocol = sqlx::query_as::<_, ProtocolRow>(
        "SELECT id, user_id, name, description, start_date, duration_days,
                status, is_template, tags, source_url,
                share_token, share_expires_at, created_at
         FROM protocols
         WHERE id = $1",
    )
    .bind(protocol_id)
    .fetch_one(pool)
    .await?;

    let lines = fetch_lines_with_doses(pool, protocol_id).await?;

    Ok(build_response(protocol, lines))
}

/// Copy a template to a user with a given start date.
pub async fn copy_template(
    pool: &PgPool,
    template_id: Uuid,
    user_id: Uuid,
    start_date: NaiveDate,
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

async fn fetch_lines_with_doses(
    pool: &PgPool,
    protocol_id: Uuid,
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
            "SELECT id, protocol_line_id, day_number, status, intervention_id, logged_at
             FROM protocol_doses
             WHERE protocol_line_id = $1
             ORDER BY day_number",
        )
        .bind(line.id)
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

fn build_response(protocol: ProtocolRow, lines: Vec<ProtocolLineResponse>) -> ProtocolResponse {
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
    }
}
