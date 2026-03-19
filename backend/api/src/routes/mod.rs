// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Route handlers.
//!
//! Each file in this module defines a function returning an Axum `Router`.
//! Route groups: auth, health_records, interventions, checkins, observations,
//! labs, timeline, export, integrations, cooperative, waitlist.

pub mod waitlist;
