// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Background sync jobs.
//!
//! Tokio background tasks — one file per integration sync job.
//! Jobs: Google Calendar sync, Garmin sync, Oura sync, Dexcom sync (Phase 2).

pub mod garmin_sync;
pub mod insight_generator;
pub mod mychart_sync;
pub mod oura_sync;

use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

/// Spawn the insight generation background job that runs every 6 hours.
/// Returns the task handle so callers (and tests) can observe shutdown;
/// `main.rs` does not need to await it.
pub fn spawn_insight_job(pool: PgPool, cancel: CancellationToken) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(6 * 60 * 60));
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
