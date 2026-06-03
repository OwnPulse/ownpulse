// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Charts
import SwiftUI

struct WeeklySummaryCard: View {
    let summary: DashboardSummary

    // MARK: C7 chart
    /// Weekly activity bars, shaped by the testable transform and colored from
    /// B5's shared token source so each category matches its web color.
    private var bars: [DashboardChartData.WeeklyBar] {
        DashboardChartData.weeklyBars(from: summary)
    }

    private func color(for bar: DashboardChartData.WeeklyBar) -> Color {
        ChartColors.color(for: bar.colorMetric, index: bar.colorIndex)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("This Week")
                .font(.headline)
                .foregroundStyle(.primary)

            // MARK: C7 chart — weekly activity bar chart
            Chart(bars) { bar in
                BarMark(
                    x: .value("Category", bar.label),
                    y: .value("Count", bar.value)
                )
                .foregroundStyle(color(for: bar))
                .cornerRadius(4)
                .annotation(position: .top, alignment: .center) {
                    Text("\(bar.value)")
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(.secondary)
                }
            }
            .chartYAxis(.hidden)
            .chartXAxis {
                AxisMarks { value in
                    AxisValueLabel {
                        if let label = value.as(String.self) {
                            Text(label)
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
            .frame(height: 140)
            .accessibilityIdentifier("weeklySummaryChart")
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("This week's activity counts")
            .accessibilityValue(DashboardChartData.weeklyAccessibilitySummary(from: summary))
        }
        .opCard()
    }
}
