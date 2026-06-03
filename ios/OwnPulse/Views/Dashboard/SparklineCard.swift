// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Charts
import SwiftUI

struct SparklineCard: View {
    let series: SeriesData

    private var displayName: String {
        series.field.replacingOccurrences(of: "_", with: " ").capitalized
    }

    private var latestValue: String {
        guard let last = series.points.last else { return "--" }
        return String(format: "%.0f", last.v)
    }

    private var trendArrow: (symbol: String, color: Color) {
        let points = series.points
        guard points.count >= 2 else { return ("arrow.forward", OPColor.trendFlat) }

        let recent = points.suffix(3).map(\.v)
        let avg = recent.reduce(0, +) / Double(recent.count)
        let previousAvg = points.prefix(max(1, points.count - 3)).map(\.v)
        let prevAvg = previousAvg.isEmpty ? avg : previousAvg.reduce(0, +) / Double(previousAvg.count)

        let delta = avg - prevAvg
        if delta > 0.5 {
            return ("arrow.up.right", OPColor.sage)
        } else if delta < -0.5 {
            return ("arrow.down.right", OPColor.trendUp)
        }
        return ("arrow.forward", OPColor.trendFlat)
    }

    /// Spoken trend description so VoiceOver users get the direction that the
    /// arrow icon and its colour convey visually.
    private var trendDescription: String {
        switch trendArrow.symbol {
        case "arrow.up.right": return "trending up"
        case "arrow.down.right": return "trending down"
        default: return "holding steady"
        }
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

                Image(systemName: trendArrow.symbol)
                    .font(.caption)
                    .foregroundStyle(trendArrow.color)
                    .accessibilityHidden(true)
            }

            if !series.points.isEmpty {
                Chart {
                    ForEach(Array(series.points.enumerated()), id: \.offset) { index, point in
                        LineMark(
                            x: .value("Day", index),
                            y: .value("Value", point.v)
                        )
                        .foregroundStyle(OPColor.teal)
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
        .accessibilityLabel(displayName)
        .accessibilityValue("\(latestValue), \(trendDescription)")
    }
}
