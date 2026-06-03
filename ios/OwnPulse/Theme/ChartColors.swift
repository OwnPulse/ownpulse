// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors
//
// GENERATED FILE — DO NOT EDIT BY HAND.
// Source: docs/design/tokens.json. Regenerate with `npm run build:tokens` in tools/design-tokens.

import SwiftUI

/// Per-metric chart colors, generated from the canonical token source.
/// Shares its source of truth with the web `chartColors.ts` map (including the
/// field-name alias layer), so the same metric resolves to the same color on
/// both platforms.
enum ChartColors {
    /// Colors keyed by canonical metric name.
    static let metric: [String: Color] = [
        "bp_diastolic": Color(red: 86 / 255, green: 180 / 255, blue: 233 / 255),
        "bp_systolic": Color(red: 204 / 255, green: 121 / 255, blue: 167 / 255),
        "glucose": Color(red: 0 / 255, green: 114 / 255, blue: 178 / 255),
        "heart_rate": Color(red: 213 / 255, green: 94 / 255, blue: 0 / 255),
        "hrv": Color(red: 0 / 255, green: 158 / 255, blue: 115 / 255),
        "sleep_duration": Color(red: 123 / 255, green: 97 / 255, blue: 194 / 255),
        "weight": Color(red: 196 / 255, green: 154 / 255, blue: 60 / 255),
    ]

    /// Deterministic fallback cycle for metrics without a dedicated color.
    static let fallback: [Color] = [
        Color(red: 194 / 255, green: 101 / 255, blue: 74 / 255),
        Color(red: 230 / 255, green: 159 / 255, blue: 0 / 255),
        Color(red: 86 / 255, green: 180 / 255, blue: 233 / 255),
        Color(red: 0 / 255, green: 158 / 255, blue: 115 / 255),
        Color(red: 212 / 255, green: 160 / 255, blue: 23 / 255),
        Color(red: 0 / 255, green: 114 / 255, blue: 178 / 255),
        Color(red: 213 / 255, green: 94 / 255, blue: 0 / 255),
        Color(red: 204 / 255, green: 121 / 255, blue: 167 / 255),
        Color(red: 91 / 255, green: 138 / 255, blue: 114 / 255),
        Color(red: 136 / 255, green: 204 / 255, blue: 238 / 255),
        Color(red: 68 / 255, green: 170 / 255, blue: 153 / 255),
        Color(red: 221 / 255, green: 204 / 255, blue: 119 / 255),
    ]

    /// Backend `record_type` field names that are synonyms for a canonical key.
    static let aliases: [String: String] = [
        "blood_glucose": "glucose",
        "blood_pressure_diastolic": "bp_diastolic",
        "blood_pressure_systolic": "bp_systolic",
        "body_mass": "weight",
        "heart_rate_variability": "hrv",
        "resting_heart_rate": "heart_rate",
        "sleep_analysis": "sleep_duration",
    ]

    /// Resolves a metric to its color: the keyed color when the field (or one
    /// of its aliases) has one, otherwise the fallback cycle indexed by `index`.
    static func color(for metric: String, index: Int) -> Color {
        let key = aliases[metric] ?? metric
        if let mapped = self.metric[key] {
            return mapped
        }
        return fallback[((index % fallback.count) + fallback.count) % fallback.count]
    }
}
