// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Database access layer.
//!
//! SQLx query functions live here. No business logic — just data access.

pub mod audit;
pub mod checkins;
pub mod explore;
pub mod explore_charts;
pub mod feature_flags;
pub mod friend_shares;
pub mod genetics;
pub mod health_records;
pub mod healthkit;
pub mod insights;
pub mod integration_tokens;
pub mod interventions;
pub mod invites;
pub mod lab_results;
pub mod observations;
pub mod observer_polls;
pub mod password_reset_tokens;
pub mod protocols;
pub mod refresh_tokens;
pub mod saved_medicines;
pub mod snp_seed;
pub mod source_preferences;
pub mod stats;
pub mod telemetry;
pub mod user_auth_methods;
pub mod users;
