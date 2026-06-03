// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Charts
import SwiftUI

struct SparklineCard: View {
    let series: SeriesData
    /// Position of this card in the sparkline row. Keyed metrics ignore it, but
    /// unkeyed series (e.g. the check-in scores) use it to pick a distinct color
    /// from the fallback cycle so they don't all render the same hue.
    var index: Int = 0

    private var displayName: String {
        series.field.replacingOccurrences(of: "_", with: " ").capitalized
    }

    // MARK: C7 chart
    /// Sparkline line color from B5's shared token source, keyed by the series
    /// field so the metric matches its color on the web dashboard. Unkeyed
    /// fields fall back to the deterministic cycle indexed by `index`.
    private var chartColor: Color {
        ChartColors.color(for: series.field, index: index)
    }

    private var latestValue: String {
        guard let last = series.points.last else { return "--" }
        return String(format: "%.0f", last.v)
    }

    // MARK: C9 trend
    // Direction is computed once and rendered as both an arrow (shape) and a
    // Wong colorblind-safe color via TrendDirection, so it reads in grayscale.
    private var trendDirection: TrendDirection {
        let points = series.points
        guard points.count >= 2 else { return .flat }

        let recent = points.suffix(3).map(\.v)
        let avg = recent.reduce(0, +) / Double(recent.count)
        let previousAvg = points.prefix(max(1, points.count - 3)).map(\.v)
        let prevAvg = previousAvg.isEmpty ? avg : previousAvg.reduce(0, +) / Double(previousAvg.count)

        let delta = avg - prevAvg
        if delta > 0.5 {
            return .up
        } else if delta < -0.5 {
            return .down
        }
        return .flat
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(displayName)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)

            HStack(alignment: .firstTextBaseline, spacing: 4) {
                Text(latestValue)
                    .font(.system(.title2, design: .rounded, weight: .bold))

                // MARK: C9 trend
                Image(systemName: trendDirection.systemImage)
                    .font(.caption)
                    .foregroundStyle(trendDirection.color)
                    .accessibilityHidden(true)
            }

            if !series.points.isEmpty {
                // MARK: C7 chart — sparkline rendered with the metric's token color
                Chart {
                    ForEach(Array(series.points.enumerated()), id: \.offset) { index, point in
                        LineMark(
                            x: .value("Day", index),
                            y: .value("Value", point.v)
                        )
                        .foregroundStyle(chartColor)
                        .interpolationMethod(.catmullRom)
                        .lineStyle(StrokeStyle(lineWidth: 2))
                    }
                }
                .chartXAxis(.hidden)
                .chartYAxis(.hidden)
                .frame(height: 40)
                .accessibilityHidden(true)
            }
        }
        .frame(width: 130)
        .opCard()
        .accessibilityElement(children: .ignore)
        .accessibilityIdentifier("sparkline-\(series.field)")
        .accessibilityLabel(displayName)
        .accessibilityValue("\(latestValue), \(trendDirection.spokenDescription)")
    }
}
