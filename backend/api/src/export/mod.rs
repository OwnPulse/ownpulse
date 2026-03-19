// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Streaming export logic.
//!
//! Supports JSON, CSV, and FHIR R4 export formats.
//! Never buffers the full dataset in memory — streams rows from the database
//! directly to the HTTP response.

pub mod csv;
pub mod json;
