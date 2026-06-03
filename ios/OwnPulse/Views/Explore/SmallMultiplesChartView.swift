// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Charts
import SwiftUI

/// Small-multiples chart: one `Chart` per metric, stacked vertically, each
/// auto-scaled to its own data range. Used by Health Overview where the
/// metrics have no meaningful shared scale (kg / bpm / min).
///
/// Interventions render as `RuleMark`s across every panel so correlations
/// remain visible across rows.
struct SmallMultiplesChartView: View {
    let metrics: [ChartMetric]
    let interventions: [InterventionMarker]
    let hiddenSubstances: Set<String>
    let panelHeight: CGFloat
    let showMovingAverage: Bool

    private var visibleInterventions: [InterventionMarker] {
        Self.filterVisibleInterventions(interventions, hiddenSubstances: hiddenSubstances)
    }

    /// Exposed for unit tests — hidden substances must not render as
    /// `RuleMark`s. Kept as a static pure function so tests don't have to
    /// instantiate a View. `nonisolated` because it's a pure transform and
    /// the enclosing `View` type is `@MainActor` by inference.
    nonisolated static func filterVisibleInterventions(
        _ interventions: [InterventionMarker],
        hiddenSubstances: Set<String>
    ) -> [InterventionMarker] {
        interventions.filter { !hiddenSubstances.contains($0.substance) }
    }

    /// Exposed for unit tests — body_mass panels must reflect the user's
    /// weight-unit preference in the unit label, everything else passes
    /// through the backend-supplied unit. `nonisolated` for the same reason
    /// as `filterVisibleInterventions` above.
    nonisolated static func unitLabel(
        for metric: ChartMetric,
        prefs: WeightUnitPreference = UserPreferences.weightUnit
    ) -> String {
        if metric.field == "body_mass" {
            return WeightFormatter.unitString(prefs: prefs)
        }
        return metric.unit
    }

    var body: some View {
        VStack(spacing: 12) {
            ForEach(metrics) { metric in
                panel(metric: metric)
            }
        }
        .accessibilityIdentifier("smallMultiplesChart")
    }

    @ViewBuilder
    private func panel(metric: ChartMetric) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 6) {
                Circle()
                    .fill(metric.color)
                    .frame(width: 8, height: 8)
                    .accessibilityHidden(true)
                Text(metric.label)
                    .font(.caption)
                    .fontWeight(.medium)
                    .foregroundStyle(.primary)
                Text(unitLabel(for: metric))
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                Spacer()
            }
            .padding(.leading, 4)

            chartContent(metric: metric)
                .frame(height: panelHeight)
                .accessibilityIdentifier("smallMultiple-\(metric.field)")
                .accessibilityElement(children: .ignore)
                .accessibilityLabel("\(metric.label) chart")
                .accessibilityValue(Self.panelAccessibilityValue(for: metric))
        }
    }

    /// Spoken summary of the panel's value range so VoiceOver users get the
    /// trend the line conveys visually. `nonisolated` pure function so it can
    /// be unit-tested without a SwiftUI runtime.
    nonisolated static func panelAccessibilityValue(
        for metric: ChartMetric,
        prefs: WeightUnitPreference = UserPreferences.weightUnit
    ) -> String {
        let unit = unitLabel(for: metric, prefs: prefs)
        guard let first = metric.points.first?.value,
              let last = metric.points.last?.value else {
            return "No data"
        }
        // body_mass values arrive in kg — convert to the user's unit so the
        // spoken value matches the unit label and the visible axis.
        let fmt: (Double) -> String = { value in
            if metric.field == "body_mass" {
                return WeightFormatter.formatValueOnly(kg: value, prefs: prefs)
            }
            return String(format: "%.1f", value)
        }
        return "From \(fmt(first)) to \(fmt(last)) \(unit)"
    }

    @ViewBuilder
    private func chartContent(metric: ChartMetric) -> some View {
        let isBodyMass = metric.field == "body_mass"
        let weightPrefs = UserPreferences.weightUnit

        Chart {
            ForEach(metric.points) { point in
                LineMark(
                    x: .value("Date", point.date),
                    y: .value(metric.label, point.value),
                    series: .value("Metric", metric.field)
                )
                .foregroundStyle(metric.color)
                .interpolationMethod(.catmullRom)
                .lineStyle(StrokeStyle(lineWidth: 2))
            }

            if showMovingAverage, let maPoints = metric.maPoints {
                ForEach(maPoints) { point in
                    LineMark(
                        x: .value("Date", point.date),
                        y: .value("\(metric.label) MA", point.value),
                        series: .value("Metric", "\(metric.field)-ma")
                    )
                    .foregroundStyle(metric.color.opacity(0.5))
                    .interpolationMethod(.catmullRom)
                    .lineStyle(StrokeStyle(lineWidth: 1.25, dash: [5, 3]))
                }
            }

            ForEach(visibleInterventions) { marker in
                RuleMark(x: .value("Intervention", marker.date))
                    .foregroundStyle(OPColor.gold.opacity(0.45))
                    .lineStyle(StrokeStyle(lineWidth: 1, dash: [4, 3]))
            }
        }
        .chartYScale(domain: .automatic(includesZero: ChartAxisConfig.includesZeroInYAxis))
        .chartYAxis {
            AxisMarks(position: .leading, values: .automatic(desiredCount: 3)) { value in
                AxisGridLine(stroke: StrokeStyle(lineWidth: 0.5, dash: [4]))
                    .foregroundStyle(.secondary.opacity(0.3))
                if isBodyMass {
                    AxisValueLabel {
                        if let kg = value.as(Double.self) {
                            Text(WeightFormatter.formatValueOnly(kg: kg, prefs: weightPrefs))
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                } else {
                    AxisValueLabel()
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .chartXAxis {
            AxisMarks(values: .automatic(desiredCount: 4)) { _ in
                AxisGridLine(stroke: StrokeStyle(lineWidth: 0.5, dash: [4]))
                    .foregroundStyle(.secondary.opacity(0.2))
                AxisValueLabel(format: .dateTime.month(.abbreviated).day())
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private func unitLabel(for metric: ChartMetric) -> String {
        Self.unitLabel(for: metric)
    }
}
