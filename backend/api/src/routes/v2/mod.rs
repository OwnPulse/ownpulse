// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! API v2 route group.
//!
//! This is the scaffold for the next API version. It is intentionally empty:
//! no routes are registered yet. v2 endpoints are added here only when a
//! breaking change to a `v1` endpoint is required (see the "API versioning
//! policy" section in `docs/architecture/api.md`).
//!
//! Until a v2 endpoint exists, requests under `/api/v2/*` fall through to a
//! clean 404 — the namespace is mounted but serves nothing.

use axum::Router;

use crate::AppState;

/// Build the (currently empty) `/api/v2` route group.
///
/// Mounted under `/api/v2` by `build_app`. Add `.route(...)` calls here as v2
/// endpoints are introduced.
pub fn router() -> Router<AppState> {
    Router::new()
}
