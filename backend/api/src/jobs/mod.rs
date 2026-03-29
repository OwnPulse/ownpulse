// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Background sync jobs.
//!
//! Tokio background tasks — one file per integration sync job.
//! Jobs: Google Calendar sync, Garmin sync, Oura sync, Dexcom sync (Phase 2).

<<<<<<< HEAD
pub mod insight_generator;

use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

/// Spawn the insight generation background job that runs every 6 hours.
pub fn spawn_insight_job(pool: PgPool, cancel: CancellationToken) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(6 * 60 * 60));
        // Skip the first immediate tick — let the server warm up.
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
    });
}
=======
pub mod garmin_sync;
pub mod oura_sync;
>>>>>>> 85867c6 (feat(backend): add Garmin and Oura integrations with OAuth and sync jobs)
