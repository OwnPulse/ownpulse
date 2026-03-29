// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Insight generation logic.
//!
//! Analyzes a user's recent data and produces insight cards (trends, anomalies,
//! streaks, missing data). Called by the background job and the manual trigger
//! endpoint.

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::insights;
use crate::models::insight::InsightRow;

/// Generate all insights for a single user and return the newly created ones.
pub async fn generate_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<InsightRow>, sqlx::Error> {
    let mut generated = Vec::new();

    // Clean up stale insights first (older than 30 days).
    insights::delete_stale(pool, 30).await?;

    // Run each generator, collecting new insights.
    generated.extend(generate_trend_insights(pool, user_id).await?);
    generated.extend(generate_anomaly_insights(pool, user_id).await?);
    generated.extend(generate_missing_data_insights(pool, user_id).await?);
    generated.extend(generate_streak_insights(pool, user_id).await?);

    Ok(generated)
}

/// Trend insights: for each check-in dimension, compare recent 7-day avg vs
/// earlier 7-day avg within a 30-day window. Generate insight if change >= 10%.
async fn generate_trend_insights(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<InsightRow>, sqlx::Error> {
    let mut results = Vec::new();

    let dimensions = ["energy", "mood", "focus", "recovery", "libido"];

    for dim in &dimensions {
        let dedup_key = format!("checkins.{dim}");

        if insights::exists_recent(pool, user_id, "trend", "dedup_key", &dedup_key, 7).await? {
            continue;
        }

        let row = sqlx::query_as::<_, TrendRow>(&format!(
            "WITH recent AS (
                SELECT {dim} AS val, date
                FROM daily_checkins
                WHERE user_id = $1
                  AND {dim} IS NOT NULL
                  AND date >= CURRENT_DATE - INTERVAL '30 days'
                ORDER BY date
            ),
            early AS (
                SELECT AVG(val)::float8 AS avg_val
                FROM recent
                WHERE date < CURRENT_DATE - INTERVAL '23 days'
            ),
            late AS (
                SELECT AVG(val)::float8 AS avg_val
                FROM recent
                WHERE date >= CURRENT_DATE - INTERVAL '7 days'
            )
            SELECT
                early.avg_val AS early_avg,
                late.avg_val AS late_avg,
                (SELECT COUNT(*) FROM recent) AS total_count
            FROM early, late"
        ))
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = row {
            let (early_avg, late_avg, count) = (row.early_avg, row.late_avg, row.total_count);

            if let (Some(early), Some(late), Some(count)) = (early_avg, late_avg, count) {
                if early.abs() < f64::EPSILON || count < 7 {
                    continue;
                }

                let change_pct = ((late - early) / early) * 100.0;

                if change_pct.abs() >= 10.0 {
                    let direction = if change_pct > 0.0 { "up" } else { "down" };
                    let headline = format!(
                        "Your {} scores have been trending {} {:.0}% over the past 30 days",
                        dim,
                        direction,
                        change_pct.abs()
                    );
                    let detail = Some(format!(
                        "Average {} went from {:.1} to {:.1} (based on {} check-ins)",
                        dim, early, late, count
                    ));
                    let metadata = json!({
                        "metric_source": "checkins",
                        "metric_field": dim,
                        "change_pct": (change_pct * 10.0).round() / 10.0,
                        "period_days": 30,
                        "dedup_key": dedup_key,
                        "explore_params": {
                            "source": "checkins",
                            "field": dim,
                            "preset": "30d"
                        }
                    });

                    let insight = insights::insert(
                        pool,
                        user_id,
                        "trend",
                        &headline,
                        detail.as_deref(),
                        &metadata,
                    )
                    .await?;
                    results.push(insight);
                }
            }
        }
    }

    Ok(results)
}

#[derive(sqlx::FromRow)]
struct TrendRow {
    early_avg: Option<f64>,
    late_avg: Option<f64>,
    total_count: Option<i64>,
}

/// Anomaly insights: for each health_record type, check if the most recent value
/// is more than 2 standard deviations from the 30-day mean.
async fn generate_anomaly_insights(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<InsightRow>, sqlx::Error> {
    let mut results = Vec::new();

    // Get distinct record types the user tracks
    let record_types: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT DISTINCT record_type, unit
         FROM health_records
         WHERE user_id = $1
           AND value IS NOT NULL
           AND start_time >= now() - INTERVAL '30 days'",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    for (record_type, unit) in &record_types {
        let dedup_key = format!("health_records.{record_type}");

        if insights::exists_recent(pool, user_id, "anomaly", "dedup_key", &dedup_key, 7).await? {
            continue;
        }

        let stats = sqlx::query_as::<_, AnomalyRow>(
            "WITH stats AS (
                SELECT
                    AVG(value) AS mean,
                    STDDEV_POP(value) AS stddev,
                    COUNT(*) AS cnt
                FROM health_records
                WHERE user_id = $1
                  AND record_type = $2
                  AND value IS NOT NULL
                  AND start_time >= now() - INTERVAL '30 days'
            ),
            latest AS (
                SELECT value
                FROM health_records
                WHERE user_id = $1
                  AND record_type = $2
                  AND value IS NOT NULL
                ORDER BY start_time DESC
                LIMIT 1
            )
            SELECT stats.mean, stats.stddev, stats.cnt, latest.value AS latest_value
            FROM stats, latest",
        )
        .bind(user_id)
        .bind(record_type)
        .fetch_optional(pool)
        .await?;

        if let Some(stats) = stats {
            let (mean, stddev, count, latest) =
                (stats.mean, stats.stddev, stats.cnt, stats.latest_value);

            if let (Some(mean), Some(stddev), Some(count), Some(latest)) =
                (mean, stddev, count, latest)
            {
                if count < 7 || stddev < f64::EPSILON {
                    continue;
                }

                let std_devs = (latest - mean) / stddev;

                if std_devs.abs() > 2.0 {
                    let direction = if std_devs > 0.0 { "high" } else { "low" };
                    let unit_str = unit.as_deref().unwrap_or("");
                    let headline = format!(
                        "Your {} yesterday ({:.0} {}) was unusually {}",
                        record_type.replace('_', " "),
                        latest,
                        unit_str,
                        direction
                    );
                    let detail = Some(format!(
                        "{:.1} standard deviations {} your 30-day average of {:.0} {}",
                        std_devs.abs(),
                        if std_devs > 0.0 { "above" } else { "below" },
                        mean,
                        unit_str
                    ));
                    let metadata = json!({
                        "metric_source": "health_records",
                        "metric_field": record_type,
                        "value": latest,
                        "avg_30d": (mean * 10.0).round() / 10.0,
                        "std_devs": (std_devs.abs() * 10.0).round() / 10.0,
                        "dedup_key": dedup_key,
                    });

                    let insight = insights::insert(
                        pool,
                        user_id,
                        "anomaly",
                        &headline,
                        detail.as_deref(),
                        &metadata,
                    )
                    .await?;
                    results.push(insight);
                }
            }
        }
    }

    Ok(results)
}

#[derive(sqlx::FromRow)]
struct AnomalyRow {
    mean: Option<f64>,
    stddev: Option<f64>,
    cnt: Option<i64>,
    latest_value: Option<f64>,
}

/// Missing data insights: notify if the user hasn't logged a check-in recently
/// but has historically logged at least 5.
async fn generate_missing_data_insights(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<InsightRow>, sqlx::Error> {
    let mut results = Vec::new();

    if insights::exists_recent(
        pool,
        user_id,
        "missing_data",
        "dedup_key",
        "checkins.missing",
        7,
    )
    .await?
    {
        return Ok(results);
    }

    let row: Option<(Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM daily_checkins WHERE user_id = $1) AS total,
            (SELECT EXTRACT(DAY FROM now() - MAX(date))::bigint
             FROM daily_checkins WHERE user_id = $1) AS days_since",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    if let Some((Some(total), Some(days_since))) = row
        && total >= 5
        && days_since >= 3
    {
        let headline = format!("You haven't logged a check-in in {} days", days_since);
        let detail = Some("Regular check-ins help track patterns over time.".to_string());
        let metadata = json!({
            "days_since": days_since,
            "dedup_key": "checkins.missing",
        });

        let insight = insights::insert(
            pool,
            user_id,
            "missing_data",
            &headline,
            detail.as_deref(),
            &metadata,
        )
        .await?;
        results.push(insight);
    }

    Ok(results)
}

/// Streak insights: celebrate consecutive days of check-in logging.
async fn generate_streak_insights(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<InsightRow>, sqlx::Error> {
    let mut results = Vec::new();

    // Count consecutive days with check-ins ending today (or yesterday).
    // ROW_NUMBER() produces bigint; cast to int4 so date arithmetic works.
    let streak: Option<i64> = sqlx::query_scalar(
        "WITH dates AS (
            SELECT DISTINCT date
            FROM daily_checkins
            WHERE user_id = $1
              AND date <= CURRENT_DATE
            ORDER BY date DESC
        ),
        numbered AS (
            SELECT date, (ROW_NUMBER() OVER (ORDER BY date DESC))::int AS rn
            FROM dates
        ),
        streak AS (
            SELECT date, rn, date + rn AS grp
            FROM numbered
        )
        SELECT COUNT(*)::bigint AS streak_len
        FROM streak
        WHERE grp = (SELECT grp FROM streak WHERE rn = 1)",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    if let Some(Some(streak_len)) = streak.map(Some) {
        // Only celebrate streaks >= 7 that are multiples of 7.
        if streak_len >= 7 && streak_len % 7 == 0 {
            let dedup_key = format!("streak.{streak_len}");
            if insights::exists_recent(pool, user_id, "streak", "dedup_key", &dedup_key, 7).await? {
                return Ok(results);
            }

            let headline = format!(
                "You've logged check-ins for {} consecutive days!",
                streak_len
            );
            let metadata = json!({
                "streak_days": streak_len,
                "dedup_key": format!("streak.{streak_len}"),
            });

            let insight =
                insights::insert(pool, user_id, "streak", &headline, None, &metadata).await?;
            results.push(insight);
        }
    }

    Ok(results)
}

/// Background job entry point: generate insights for all active users.
pub async fn run_for_all_users(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let user_ids: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE status = 'active'")
        .fetch_all(pool)
        .await?;

    let mut total = 0u64;
    for (user_id,) in &user_ids {
        match generate_for_user(pool, *user_id).await {
            Ok(insights) => {
                total += insights.len() as u64;
            }
            Err(err) => {
                tracing::error!(user_id = %user_id, error = %err, "failed to generate insights");
            }
        }
    }

    Ok(total)
}
