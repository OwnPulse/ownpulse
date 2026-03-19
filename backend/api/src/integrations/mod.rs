// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! HTTP clients for external APIs.
//!
//! One module per data source. All clients are designed for WireMock
//! compatibility in tests — they accept a base URL parameter.
//! Modules: healthkit_write, garmin, oura, dexcom, lab_pdf, genetics.
