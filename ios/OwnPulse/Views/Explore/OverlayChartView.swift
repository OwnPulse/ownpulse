// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Charts
import SwiftUI

struct ChartPoint: Identifiable, Sendable {
    let date: Date
    let value: Double
    var id: Double { date.timeIntervalSince1970 }
}

struct ChartMetric: Identifiable {
    let field: String
    let label: String
    let unit: String
    let color: Color
    let points: [ChartPoint]
    let maPoints: [ChartPoint]?
    var id: String { field }
}

struct OverlayChartView: View {
    let metrics: [ChartMetric]
    let interventions: [InterventionMarker]
    let hiddenSubstances: Set<String>
    let height: CGFloat
    let showMovingAverage: Bool

    private var visibleInterventions: [InterventionMarker] {
        interventions.filter { !hiddenSubstances.contains($0.substance) }
    }

    /// Group metrics by unit for dual-axis support.
    /// First unit group goes on the left y-axis, second on the right.
    private var unitGroups: (left: [ChartMetric], right: [ChartMetric]) {
        var seen: [String] = []
        for metric in metrics {
            if !seen.contains(metric.unit) {
                seen.append(metric.unit)
            }
        }
        let leftUnit = seen.first ?? ""
        let left = metrics.filter { $0.unit == leftUnit }
        let right = metrics.filter { $0.unit != leftUnit }
        return (left, right)
    }

    var body: some View {
        let groups = unitGroups

        ZStack {
            // Left axis chart
            if !groups.left.isEmpty {
                chartLayer(metrics: groups.left, axisPosition: .leading)
            }

            // Right axis chart (overlaid with transparent background)
            if !groups.right.isEmpty {
                chartLayer(metrics: groups.right, axisPosition: .trailing)
            }
        }
        .frame(height: height)
        .accessibilityIdentifier("overlayChart")
    }

    @ViewBuilder
    private func chartLayer(metrics layerMetrics: [ChartMetric], axisPosition: AxisMarkPosition) -> some View {
        Chart {
            ForEach(layerMetrics) { metric in
                ForEach(metric.points) { point in
                    LineMark(
                        x: .value("Date", point.date),
                        y: .value(metric.label, point.value),
                        series: .value("Metric", metric.field)
                    )
                    .foregroundStyle(metric.color)
                    .interpolationMethod(.catmullRom)
                    .lineStyle(StrokeStyle(lineWidth: 2.5))

                    AreaMark(
                        x: .value("Date", point.date),
                        y: .value(metric.label, point.value),
                        series: .value("Metric", metric.field)
                    )
                    .foregroundStyle(
                        LinearGradient(
                            colors: [metric.color.opacity(0.2), metric.color.opacity(0.02)],
                            startPoint: .top,
                            endPoint: .bottom
                        )
                    )
                    .interpolationMethod(.catmullRom)
                }

                // Moving average overlay
                if showMovingAverage, let maPoints = metric.maPoints {
                    ForEach(maPoints) { point in
                        LineMark(
                            x: .value("Date", point.date),
                            y: .value("\(metric.label) MA", point.value),
                            series: .value("Metric", "\(metric.field)-ma")
                        )
                        .foregroundStyle(metric.color.opacity(0.5))
                        .interpolationMethod(.catmullRom)
                        .lineStyle(StrokeStyle(lineWidth: 1.5, dash: [6, 4]))
                    }
                }
            }

            // Intervention markers
            ForEach(visibleInterventions) { marker in
                RuleMark(x: .value("Intervention", marker.date))
                    .foregroundStyle(OPColor.gold.opacity(0.6))
                    .lineStyle(StrokeStyle(lineWidth: 1, dash: [4, 3]))
                    .annotation(position: .top, alignment: .center) {
                        Text(marker.substance)
                            .font(.system(size: 9, weight: .medium))
                            .foregroundStyle(OPColor.gold)
                            .lineLimit(1)
                            .padding(.horizontal, 4)
                            .padding(.vertical, 2)
                            .background(
                                Capsule()
                                    .fill(OPColor.gold.opacity(0.15))
                            )
                    }
            }
        }
        .chartScrollableAxes(.horizontal)
        .chartYAxis {
            AxisMarks(position: axisPosition, values: .automatic(desiredCount: 4)) { value in
                AxisGridLine(stroke: StrokeStyle(lineWidth: 0.5, dash: [4]))
                    .foregroundStyle(.secondary.opacity(0.3))
                AxisValueLabel()
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
        .chartXAxis {
            AxisMarks(values: .automatic(desiredCount: 5)) { _ in
                AxisGridLine(stroke: StrokeStyle(lineWidth: 0.5, dash: [4]))
                    .foregroundStyle(.secondary.opacity(0.2))
                AxisValueLabel(format: .dateTime.month(.abbreviated).day())
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
    }
}
