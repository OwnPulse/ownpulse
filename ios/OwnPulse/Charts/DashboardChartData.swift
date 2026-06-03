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
    /// The canonical hero-metric backend field, pinned in ONE place. Both the
    /// view model (`DashboardViewModel.heroMetricFieldKey`) and the card's
    /// parameter default derive their default from this so they can't drift.
    /// `resting_heart_rate` aliases to the `heart_rate` token color in
    /// `ChartColors`, so the hero card shows the heart-rate color out of the box.
    static let defaultHeroField = "resting_heart_rate"

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
        // "Records" is keyed to the heart_rate token color (the most common
        // health record); the other three categories have no dedicated token,
        // so they deliberately cycle through the fallback palette by index.
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

    /// Resolves the metric key used for color lookup from a hero metric's
    /// backend field name. An empty field (no hero series loaded yet) falls
    /// back to the canonical default; otherwise the field is passed straight to
    /// `ChartColors.color(for:index:)`, which resolves it through the alias
    /// layer (e.g. `resting_heart_rate` -> the `heart_rate` token color).
    static func colorKey(forField field: String) -> String {
        field.isEmpty ? defaultHeroField : field
    }
}
