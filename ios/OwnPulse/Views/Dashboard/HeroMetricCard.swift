// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Charts
import SwiftUI

struct HeroMetricCard: View {
    let metricName: String
    let currentValue: String
    let unit: String
    let trendText: String
    /// The literal DATA direction of the change (did the value go up or down),
    /// NOT a good/bad polarity. Drives the arrow so the grayscale signal always
    /// matches the number shown in `trendText`.
    let trendDirection: TrendDirection
    let dataPoints: [DataPoint]

    @State private var animateChart = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Value and trend
            HStack(alignment: .firstTextBaseline) {
                Text(currentValue)
                    .font(.system(.largeTitle, design: .rounded, weight: .bold))
                    .foregroundStyle(OPColor.terracotta)

                Text(unit)
                    .font(.title3)
                    .foregroundStyle(.secondary)

                Spacer()

                if !trendText.isEmpty {
                    // MARK: C9 trend
                    // Arrow + Wong colorblind-safe color, both keyed off the
                    // literal data direction so the arrow always matches the
                    // sign of the number. Direction is never color alone.
                    HStack(spacing: 3) {
                        Image(systemName: trendDirection.systemImage)
                            .accessibilityHidden(true)
                        Text(trendText)
                    }
                    .font(.caption)
                    .fontWeight(.medium)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 4)
                    .background(Capsule().fill(trendDirection.color.opacity(0.2)))
                    .foregroundStyle(trendDirection.color)
                    .accessibilityIdentifier("heroTrendBadge")
                }
            }

            Text(metricName)
                .font(.subheadline)
                .foregroundStyle(.secondary)

            // 30-day chart
            Chart {
                ForEach(Array(dataPoints.enumerated()), id: \.offset) { index, point in
                    LineMark(
                        x: .value("Day", index),
                        y: .value("Value", animateChart ? point.v : 0)
                    )
                    .foregroundStyle(
                        LinearGradient(
                            colors: [OPColor.terracotta, OPColor.terracotta.opacity(0.7)],
                            startPoint: .leading,
                            endPoint: .trailing
                        )
                    )
                    .interpolationMethod(.catmullRom)
                    .lineStyle(StrokeStyle(lineWidth: 2.5))

                    AreaMark(
                        x: .value("Day", index),
                        y: .value("Value", animateChart ? point.v : 0)
                    )
                    .foregroundStyle(
                        LinearGradient(
                            colors: [
                                OPColor.terracotta.opacity(0.3),
                                OPColor.terracotta.opacity(0.05),
                            ],
                            startPoint: .top,
                            endPoint: .bottom
                        )
                    )
                    .interpolationMethod(.catmullRom)
                }
            }
            .chartXAxis(.hidden)
            .chartYAxis {
                AxisMarks(position: .leading, values: .automatic(desiredCount: 3)) { value in
                    AxisGridLine(stroke: StrokeStyle(lineWidth: 0.5, dash: [4]))
                        .foregroundStyle(.secondary.opacity(0.3))
                    AxisValueLabel()
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }
            .frame(height: 160)
            .accessibilityIdentifier("heroChart")
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("\(metricName) 30-day trend")
            .accessibilityValue("Currently \(currentValue) \(unit)\(trendText.isEmpty ? "" : ", \(trendText)")")
        }
        .opCard()
        .onAppear {
            withAnimation(.spring(duration: 0.8, bounce: 0.2).delay(0.1)) {
                animateChart = true
            }
        }
    }
}
