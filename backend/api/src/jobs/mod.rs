// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Background sync jobs.
//!
//! Tokio background tasks — one file per integration sync job.
//! Jobs: Google Calendar sync, Garmin sync, Oura sync.

pub mod garmin_sync;
pub mod google_calendar_sync;
pub mod insight_generator;
pub mod mychart_sync;
pub mod oura_sync;

use sqlx::PgPool;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use uuid::Uuid;

/// Spawn the insight generation background job that runs every 6 hours.
/// Returns the task handle so callers (and tests) can observe shutdown;
/// `main.rs` does not need to await it.
pub fn spawn_insight_job(pool: PgPool, cancel: CancellationToken) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(6 * 60 * 60));
        // `Burst` (the default) fires repeated back-to-back ticks to catch up
        // after a missed tick (e.g. the process was paused/slow) — `Delay`
        // just resumes the normal cadence from whenever we next poll, which
        // is what we want for a periodic job, not an at-least-N-times one.
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        interval.tick().await;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("insight generation job shutting down");
                    return;
                }
                _ = interval.tick() => {
                    info!("running insight generation for all users");
                    match insight_generator::run_for_all_users(&pool).await {
                        Ok(count) => {
                            info!(insights_generated = count, "insight generation complete");
                        }
                        Err(err) => {
                            error!(error = %err, "insight generation job failed");
                        }
                    }
                }
            }
        }
    })
}

/// Typed outcome of a manual (`sync_user_now`) sync attempt, so route
/// handlers map to an HTTP status by matching the variant rather than
/// comparing error strings.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// The server operator hasn't configured this integration
    /// (`GARMIN_CLIENT_ID`/`GARMIN_CLIENT_SECRET` or the Oura equivalent are
    /// unset) — self-hosters need to know this is a deployment gap, not a
    /// per-user problem.
    #[error(
        "this integration is not configured on this server — the operator needs to set the \
         corresponding client id/secret environment variables"
    )]
    NotConfigured,
    /// The calling user hasn't connected this integration.
    #[error("integration is not connected")]
    NotConnected,
    /// Too soon since the last attempt (manual or scheduled), or another sync
    /// for this user is already running (advisory lock held elsewhere).
    #[error("try again in {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },
    /// The upstream provider (or our own DB layer) failed. The message is a
    /// short, sanitized description — never a raw upstream response body.
    #[error("{0}")]
    Upstream(String),
}

/// Run `f` while holding a Postgres advisory lock scoped to `(source,
/// user_id)`, for the lifetime of one Postgres transaction on a dedicated
/// connection checked out from `pool`. Returns `Ok(None)` without running `f`
/// if another holder already has the lock — this is a *skip*, not a wait, so
/// callers should treat it as "someone else is already syncing this user"
/// rather than an error.
///
/// This exists to make per-user sync mutually exclusive:
/// - a manual sync (`POST /integrations/<source>/sync`) racing the periodic
///   job for the same user, and
/// - two replicas (`replicaCount: 2` in the Helm chart) each running their
///   own periodic job and iterating the same `integration_tokens` rows.
///
/// `pg_try_advisory_xact_lock` releases automatically when the transaction
/// ends (commit or rollback) — we don't write anything through `tx`, so a
/// plain commit is the release, whether `f` succeeded or not.
pub async fn try_with_user_sync_lock<F, Fut, T>(
    pool: &PgPool,
    source: &str,
    user_id: Uuid,
    f: F,
) -> Result<Option<T>, sqlx::Error>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let mut tx = pool.begin().await?;

    let (acquired,): (bool,) =
        sqlx::query_as("SELECT pg_try_advisory_xact_lock(hashtext($1), hashtext($2))")
            .bind(source)
            .bind(user_id.to_string())
            .fetch_one(&mut *tx)
            .await?;

    if !acquired {
        tx.rollback().await?;
        return Ok(None);
    }

    let result = f().await;
    tx.commit().await?;
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test for the `main.rs` wiring: `spawn_insight_job` must respect
    /// an already-cancelled token and return promptly rather than waiting for
    /// the 6-hour interval. Uses a lazy pool — cancellation is checked before
    /// any query would run, so no real database is needed.
    #[tokio::test]
    async fn spawn_insight_job_shuts_down_promptly_on_cancellation() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://user:pass@localhost/db")
            .expect("lazy pool construction should not touch the network");
        let cancel = CancellationToken::new();
        cancel.cancel();

        let handle = spawn_insight_job(pool, cancel);

        tokio::time::timeout(std::time::Duration::from_secs(2), handle)
            .await
            .expect("job should shut down promptly once cancelled")
            .expect("job task should not panic");
    }
}
