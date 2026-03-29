// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Database queries for the stats/correlation endpoints.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Row representing the first and last dose of a substance for a user.
#[derive(Debug)]
pub struct DoseRange {
    pub first_dose: DateTime<Utc>,
    pub last_dose: DateTime<Utc>,
}

/// Find the first and last administered_at for a substance (case-insensitive).
///
/// Returns `None` if no interventions match.
pub async fn intervention_dose_range(
    pool: &PgPool,
    user_id: Uuid,
    substance: &str,
) -> Result<Option<DoseRange>, sqlx::Error> {
    // MIN/MAX always return a single row; the values are NULL when no rows match.
    // We query into Option<DateTime> to handle the NULL case.
    let row: (Option<DateTime<Utc>>, Option<DateTime<Utc>>) = sqlx::query_as(
        "SELECT MIN(administered_at), MAX(administered_at)
         FROM interventions
         WHERE user_id = $1 AND LOWER(substance) = LOWER($2)",
    )
    .bind(user_id)
    .bind(substance)
    .fetch_one(pool)
    .await?;

    match row {
        (Some(first), Some(last)) => Ok(Some(DoseRange {
            first_dose: first,
            last_dose: last,
        })),
        _ => Ok(None),
    }
}
