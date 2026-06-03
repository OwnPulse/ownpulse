// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

// MARK: - C7 chart data transforms

/// Pure data transforms backing the Phase 3b Swift Charts dashboard cards.
///
/// These live here (rather than inside the SwiftUI views) so the chart-shaping
/// logic is unit-testable without a simulator. Views consume the resulting
/// value types and bind them straight into `Charts` marks.
enum DashboardChartData {
    /// A single bar in the ``WeeklySummaryCard`` bar chart: one activity
    /// category and its 7-day count. `colorMetric` is the canonical metric key
    /// passed to `ChartColors.color(for:index:)` so the bar tint comes from the
    /// shared token source rather than a hardcoded value.
    struct WeeklyBar: Identifiable, Equatable {
        let label: String
        let value: Int
        let colorMetric: String
        let colorIndex: Int

        var id: String { label }
    }

    /// Builds the four-category weekly summary series from a ``DashboardSummary``.
    /// Order is stable so the chart and its accessibility summary line up.
    static func weeklyBars(from summary: DashboardSummary) -> [WeeklyBar] {
        [
            WeeklyBar(label: "Check-ins", value: summary.checkinCount7d, colorMetric: "checkins", colorIndex: 0),
            WeeklyBar(label: "Records", value: summary.healthRecordCount7d, colorMetric: "heart_rate", colorIndex: 1),
            WeeklyBar(label: "Interventions", value: summary.interventionCount7d, colorMetric: "interventions", colorIndex: 2),
            WeeklyBar(label: "Observations", value: summary.observationCount7d, colorMetric: "observations", colorIndex: 3),
        ]
    }

    /// Spoken summary of the weekly bars for VoiceOver — never relies on color
    /// alone. Produces e.g. "Check-ins 5, Records 42, Interventions 3,
    /// Observations 2".
    static func weeklyAccessibilitySummary(from summary: DashboardSummary) -> String {
        weeklyBars(from: summary)
            .map { "\($0.label) \($0.value)" }
            .joined(separator: ", ")
    }

    /// Resolves the canonical metric key used for color lookup from a hero
    /// metric's backend field name. Falls back to the field itself, which
    /// `ChartColors.color(for:index:)` resolves through its alias layer.
    static func colorKey(forField field: String) -> String {
        field.isEmpty ? "heart_rate" : field
    }
}
