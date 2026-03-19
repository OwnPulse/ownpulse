// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Database access layer.
//!
//! SQLx query functions live here. No business logic — just data access.
//! All queries use `sqlx::query_as!` macros for compile-time checking.
