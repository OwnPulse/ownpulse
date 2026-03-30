// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Startup migration version check.
//!
//! Compares the number of migration files on disk (compiled into the binary)
//! against the number of applied migrations in the database. If the database
//! is behind, the readiness probe returns 503 so Kubernetes does not route
//! traffic to a pod whose schema is outdated.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use sqlx::PgPool;
use tracing::{error, info, warn};

/// Number of migration files in `db/migrations/` at compile time.
///
/// When adding a new migration, update this constant. The unit test
/// `test_expected_count_matches_migration_files` verifies it stays in sync.
pub const EXPECTED_MIGRATION_COUNT: i64 = 22;

/// Shared flag indicating whether migrations are up to date.
/// `true` means the database schema matches the binary's expectations.
pub type MigrationsReady = Arc<AtomicBool>;

/// Create a new `MigrationsReady` flag, initially `false` (not ready).
pub fn new_migrations_ready() -> MigrationsReady {
    Arc::new(AtomicBool::new(false))
}

/// Result of comparing applied migrations against expected count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationCheckResult {
    pub applied: i64,
    pub expected: i64,
}

impl MigrationCheckResult {
    /// Returns `true` if the database has at least the expected number of migrations.
    pub fn is_ready(&self) -> bool {
        self.applied >= self.expected
    }
}

/// Compare the expected migration count against the database.
///
/// Checks the `_applied_migrations` table first (the custom migration runner),
/// falling back to the legacy `_sqlx_migrations` table.
pub async fn check_migrations(pool: &PgPool) -> Result<MigrationCheckResult, sqlx::Error> {
    let applied = count_applied_migrations(pool).await?;

    let result = MigrationCheckResult {
        applied,
        expected: EXPECTED_MIGRATION_COUNT,
    };

    if result.is_ready() {
        info!(
            applied = result.applied,
            expected = result.expected,
            "database migrations are up to date"
        );
    } else {
        error!(
            applied = result.applied,
            expected = result.expected,
            "database has {} migrations applied, expected {}",
            result.applied,
            result.expected
        );
    }

    Ok(result)
}

/// Count applied migrations from `_applied_migrations` or `_sqlx_migrations`.
async fn count_applied_migrations(pool: &PgPool) -> Result<i64, sqlx::Error> {
    // Try the custom tracking table first.
    let has_applied_table: bool =
        sqlx::query_scalar("SELECT to_regclass('public._applied_migrations') IS NOT NULL")
            .fetch_one(pool)
            .await?;

    if has_applied_table {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _applied_migrations")
            .fetch_one(pool)
            .await?;
        return Ok(count);
    }

    // Fall back to legacy _sqlx_migrations table.
    let has_sqlx_table: bool =
        sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations') IS NOT NULL")
            .fetch_one(pool)
            .await?;

    if has_sqlx_table {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(pool)
            .await?;
        warn!(
            count,
            "using legacy _sqlx_migrations table for migration count"
        );
        return Ok(count);
    }

    // No migration tracking table at all — database is completely fresh.
    Ok(0)
}

/// Run the migration check and update the readiness flag.
pub async fn run_check_and_set_flag(pool: &PgPool, ready: &MigrationsReady) {
    match check_migrations(pool).await {
        Ok(result) => {
            ready.store(result.is_ready(), Ordering::SeqCst);
        }
        Err(err) => {
            error!(error = %err, "failed to check migration status");
            ready.store(false, Ordering::SeqCst);
        }
    }
}

/// Spawn a background task that re-checks migration status every few seconds
/// until the database catches up. This handles the case where a Helm pre-install
/// hook runs migrations after the pod has already started — the pod will become
/// ready once the hook completes.
pub fn spawn_migration_recheck(pool: PgPool, ready: MigrationsReady) {
    tokio::spawn(async move {
        loop {
            if ready.load(Ordering::SeqCst) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            match check_migrations(&pool).await {
                Ok(result) => {
                    if result.is_ready() {
                        info!(
                            applied = result.applied,
                            expected = result.expected,
                            "migrations caught up, pod is now ready"
                        );
                        ready.store(true, Ordering::SeqCst);
                        return;
                    }
                }
                Err(err) => {
                    warn!(error = %err, "migration recheck failed, will retry");
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_result_ready_when_equal() {
        let result = MigrationCheckResult {
            applied: 13,
            expected: 13,
        };
        assert!(result.is_ready());
    }

    #[test]
    fn check_result_ready_when_ahead() {
        let result = MigrationCheckResult {
            applied: 15,
            expected: 13,
        };
        assert!(result.is_ready());
    }

    #[test]
    fn check_result_not_ready_when_behind() {
        let result = MigrationCheckResult {
            applied: 7,
            expected: 13,
        };
        assert!(!result.is_ready());
    }

    #[test]
    fn check_result_not_ready_when_zero() {
        let result = MigrationCheckResult {
            applied: 0,
            expected: 13,
        };
        assert!(!result.is_ready());
    }

    /// Verify that `EXPECTED_MIGRATION_COUNT` matches the actual number of
    /// migration files in `db/migrations/`. This catches the case where
    /// someone adds a migration but forgets to bump the constant.
    #[test]
    fn test_expected_count_matches_migration_files() {
        let migrations_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../db/migrations");
        let count = std::fs::read_dir(&migrations_dir)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", migrations_dir.display()))
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let name = entry.file_name();
                let name = name.to_str()?;
                if name.ends_with(".sql") {
                    Some(())
                } else {
                    None
                }
            })
            .count() as i64;

        assert_eq!(
            count, EXPECTED_MIGRATION_COUNT,
            "EXPECTED_MIGRATION_COUNT ({EXPECTED_MIGRATION_COUNT}) does not match \
             the number of .sql files in db/migrations/ ({count}). \
             Update EXPECTED_MIGRATION_COUNT in migration_check.rs."
        );
    }
}
