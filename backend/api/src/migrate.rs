// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use sqlx::PgPool;
use tracing::info;

/// NOTE: When adding a new migration file to db/migrations/, you MUST also add
/// it to this array. The integration test `test_migrations_array_matches_files`
/// will fail if they are out of sync.
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_init.sql",
        include_str!("../../../db/migrations/0001_init.sql"),
    ),
    (
        "0002_waitlist.sql",
        include_str!("../../../db/migrations/0002_waitlist.sql"),
    ),
    (
        "0003_auth_provider.sql",
        include_str!("../../../db/migrations/0003_auth_provider.sql"),
    ),
    (
        "0004_audit_log.sql",
        include_str!("../../../db/migrations/0004_audit_log.sql"),
    ),
    (
        "0005_refresh_token_family.sql",
        include_str!("../../../db/migrations/0005_refresh_token_family.sql"),
    ),
    (
        "0006_roles_and_sharing.sql",
        include_str!("../../../db/migrations/0006_roles_and_sharing.sql"),
    ),
    (
        "0007_row_level_security.sql",
        include_str!("../../../db/migrations/0007_row_level_security.sql"),
    ),
    (
        "0008_email_login.sql",
        include_str!("../../../db/migrations/0008_email_login.sql"),
    ),
    (
        "0009_friend_share_declined_status.sql",
        include_str!("../../../db/migrations/0009_friend_share_declined_status.sql"),
    ),
    (
        "0010_user_auth_methods.sql",
        include_str!("../../../db/migrations/0010_user_auth_methods.sql"),
    ),
    (
        "0011_fix_google_provider_subject.sql",
        include_str!("../../../db/migrations/0011_fix_google_provider_subject.sql"),
    ),
    (
        "0012_user_status.sql",
        include_str!("../../../db/migrations/0012_user_status.sql"),
    ),
    (
        "0013_invite_codes.sql",
        include_str!("../../../db/migrations/0013_invite_codes.sql"),
    ),
    (
        "0014_invite_claims.sql",
        include_str!("../../../db/migrations/0014_invite_claims.sql"),
    ),
    (
        "0015_explore_charts.sql",
        include_str!("../../../db/migrations/0015_explore_charts.sql"),
    ),
    (
        "0016_observer_polls.sql",
        include_str!("../../../db/migrations/0016_observer_polls.sql"),
    ),
    (
        "0017_password_reset_tokens.sql",
        include_str!("../../../db/migrations/0017_password_reset_tokens.sql"),
    ),
    (
        "0019_insights.sql",
        include_str!("../../../db/migrations/0019_insights.sql"),
    ),
];

#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    #[error("migration {filename} failed: {source}")]
    Apply {
        filename: String,
        source: sqlx::Error,
    },
    #[error("migration tracking query failed: {0}")]
    Tracking(#[from] sqlx::Error),
}

/// Run all pending migrations against the database.
///
/// Handles three scenarios:
/// 1. Fresh database — run all migrations
/// 2. Legacy `_sqlx_migrations` table exists — seed tracking from it
/// 3. Tables exist but no tracking — detect applied migrations by inspecting schema artifacts
pub async fn run_migrations(pool: &PgPool) -> Result<(), MigrateError> {
    info!("starting migration runner");

    // Step 1: Create tracking table
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS _applied_migrations (
            filename TEXT PRIMARY KEY,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
        );",
    )
    .execute(pool)
    .await?;

    // Step 2: Seed from legacy _sqlx_migrations if present
    if is_tracking_empty(pool).await? {
        seed_from_sqlx_migrations(pool).await?;
    }

    // Step 3: Detect already-applied migrations by schema artifacts
    if is_tracking_empty(pool).await? {
        detect_applied_migrations(pool).await?;
    }

    // Step 4: Apply pending migrations in order, each in a transaction
    for (filename, sql) in MIGRATIONS {
        let applied: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM _applied_migrations WHERE filename = $1)",
        )
        .bind(filename)
        .fetch_one(pool)
        .await?;

        if applied {
            info!(filename, "migration already applied, skipping");
            continue;
        }

        info!(filename, "applying migration");

        let mut tx = pool.begin().await?;
        sqlx::raw_sql(sql)
            .execute(&mut *tx)
            .await
            .map_err(|e| MigrateError::Apply {
                filename: filename.to_string(),
                source: e,
            })?;
        sqlx::query("INSERT INTO _applied_migrations (filename) VALUES ($1)")
            .bind(filename)
            .execute(&mut *tx)
            .await
            .map_err(|e| MigrateError::Apply {
                filename: filename.to_string(),
                source: e,
            })?;
        tx.commit().await?;

        info!(filename, "migration applied successfully");
    }

    info!("migration runner complete");
    Ok(())
}

async fn is_tracking_empty(pool: &PgPool) -> Result<bool, MigrateError> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _applied_migrations")
        .fetch_one(pool)
        .await?;
    Ok(count == 0)
}

/// Seed `_applied_migrations` from the legacy `_sqlx_migrations` table.
///
/// The legacy table has a `description` column with names like `0004_refresh_token_family`.
/// Some migrations were renumbered, so we map the descriptive suffix to the current filenames.
async fn seed_from_sqlx_migrations(pool: &PgPool) -> Result<(), MigrateError> {
    let has_sqlx_table: bool =
        sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations') IS NOT NULL")
            .fetch_one(pool)
            .await?;

    if !has_sqlx_table {
        return Ok(());
    }

    info!("found legacy _sqlx_migrations table, seeding tracking data");

    let descriptions: Vec<String> =
        sqlx::query_scalar("SELECT description FROM _sqlx_migrations ORDER BY installed_on")
            .fetch_all(pool)
            .await?;

    // Map descriptive suffix (after the number prefix) to current filename.
    // Derived from the MIGRATIONS array to avoid drift.
    for desc in &descriptions {
        // description is like "0004_refresh_token_family" — strip the leading number and underscore
        let suffix = desc
            .find('_')
            .map(|i| &desc[i + 1..])
            .unwrap_or(desc.as_str());

        // Find the matching migration by suffix (e.g., "refresh_token_family" matches
        // "0005_refresh_token_family.sql")
        let matching = MIGRATIONS.iter().find(|(name, _)| {
            name.find('_')
                .and_then(|i| name[i + 1..].strip_suffix(".sql"))
                .is_some_and(|s| s == suffix)
        });

        if let Some((filename, _)) = matching {
            sqlx::query(
                "INSERT INTO _applied_migrations (filename) VALUES ($1) ON CONFLICT DO NOTHING",
            )
            .bind(filename)
            .execute(pool)
            .await?;
            info!(legacy_desc = %desc, filename, "seeded from _sqlx_migrations");
        } else {
            info!(legacy_desc = %desc, "no mapping found for legacy migration, skipping");
        }
    }

    Ok(())
}

/// Detect which migrations have already been applied by checking for schema artifacts.
async fn detect_applied_migrations(pool: &PgPool) -> Result<(), MigrateError> {
    info!("detecting previously applied migrations from schema artifacts");

    let checks: &[(&str, &str)] = &[
        (
            "0001_init.sql",
            "SELECT to_regclass('public.users') IS NOT NULL",
        ),
        (
            "0002_waitlist.sql",
            "SELECT to_regclass('public.waitlist') IS NOT NULL",
        ),
        (
            "0003_auth_provider.sql",
            "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'auth_provider')",
        ),
        (
            "0004_audit_log.sql",
            "SELECT to_regclass('public.data_access_log') IS NOT NULL",
        ),
        (
            "0005_refresh_token_family.sql",
            "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'refresh_tokens' AND column_name = 'family_id')",
        ),
        (
            "0006_roles_and_sharing.sql",
            "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'role')",
        ),
        // RLS detection: assumes if RLS is enabled on health_records, the full migration was applied.
        (
            "0007_row_level_security.sql",
            "SELECT COALESCE((SELECT relrowsecurity FROM pg_class WHERE relname = 'health_records'), false)",
        ),
        (
            "0008_email_login.sql",
            "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'email')",
        ),
        (
            "0009_friend_share_declined_status.sql",
            "SELECT EXISTS (SELECT 1 FROM pg_enum WHERE enumlabel = 'declined')",
        ),
        (
            "0010_user_auth_methods.sql",
            "SELECT to_regclass('public.user_auth_methods') IS NOT NULL",
        ),
        (
            "0011_fix_google_provider_subject.sql",
            // This migration updates data, not schema — detect by checking if 0010 was applied
            // (it's always applied after 0010).
            "SELECT to_regclass('public.user_auth_methods') IS NOT NULL",
        ),
        (
            "0012_user_status.sql",
            "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'users' AND column_name = 'status')",
        ),
        (
            "0013_invite_codes.sql",
            "SELECT to_regclass('public.invites') IS NOT NULL",
        ),
        (
            "0014_invite_claims.sql",
            "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'invites' AND column_name = 'claimed_by')",
        ),
        (
            "0015_explore_charts.sql",
            "SELECT to_regclass('public.explore_charts') IS NOT NULL",
        ),
        (
            "0016_observer_polls.sql",
            "SELECT to_regclass('public.observer_polls') IS NOT NULL",
        ),
        (
            "0017_insights.sql",
            "SELECT to_regclass('public.insights') IS NOT NULL",
        ),
    ];

    for (filename, check_sql) in checks {
        let detected: bool = sqlx::query_scalar(check_sql).fetch_one(pool).await?;

        if detected {
            sqlx::query(
                "INSERT INTO _applied_migrations (filename) VALUES ($1) ON CONFLICT DO NOTHING",
            )
            .bind(filename)
            .execute(pool)
            .await?;
            info!(filename, "detected as already applied");
        }
    }

    Ok(())
}

/// Returns the filenames in the embedded MIGRATIONS array (for test verification).
pub fn migration_filenames() -> Vec<&'static str> {
    MIGRATIONS.iter().map(|(name, _)| *name).collect()
}
