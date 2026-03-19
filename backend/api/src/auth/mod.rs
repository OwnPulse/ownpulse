// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Authentication and authorization.
//!
//! This module will contain:
//! - JWT token issuance and verification
//! - Refresh token rotation logic
//! - Axum middleware extractor for authenticated requests
//! - Google OAuth callback handler
