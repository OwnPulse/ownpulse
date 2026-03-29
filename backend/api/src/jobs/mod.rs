// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Background sync jobs.
//!
//! Tokio background tasks — one file per integration sync job.
//! Jobs: Google Calendar sync, Garmin sync, Oura sync, Dexcom sync (Phase 2).

pub mod garmin_sync;
pub mod oura_sync;
