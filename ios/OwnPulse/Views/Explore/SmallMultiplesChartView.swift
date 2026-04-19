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
        interventions.filter { !hiddenSubstances.contains($0.substance) }
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
        }
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
        .chartYScale(domain: .automatic(includesZero: false))
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
        if metric.field == "body_mass" {
            return WeightFormatter.unitString()
        }
        return metric.unit
    }
}
