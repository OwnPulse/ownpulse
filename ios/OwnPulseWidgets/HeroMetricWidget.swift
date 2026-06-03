// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import WidgetKit

/// Shows the latest hero metric (resting HR / HRV / sleep duration). Supports
/// a lock-screen rectangular family and a home-screen small family. The
/// `Widget` configuration lives in `WidgetConfigurations.swift`; this view is
/// shared with the app target for the DEBUG snapshot harness.
struct HeroMetricWidgetView: View {
    // See TodayCheckinWidgetView: `\.widgetFamily` is read-only, so the DEBUG
    // harness passes an explicit override; production reads the real family.
    @Environment(\.widgetFamily) private var environmentFamily
    let entry: OwnPulseEntry
    var familyOverride: WidgetFamily?

    private var family: WidgetFamily { familyOverride ?? environmentFamily }
    private var snapshot: WidgetSnapshot { entry.snapshot }

    var body: some View {
        switch family {
        case .systemSmall:
            small
                .containerBackground(.fill.tertiary, for: .widget)
        default:
            rectangular
        }
    }

    private var accessibilityValue: String {
        let trend = snapshot.heroTrendText.isEmpty ? "" : ", \(snapshot.heroTrendText)"
        return "\(snapshot.heroMetricName): \(snapshot.heroMetricValue) \(snapshot.heroMetricUnit)\(trend)"
    }

    private var rectangular: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(snapshot.heroMetricName)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)
            HStack(alignment: .firstTextBaseline, spacing: 4) {
                Text(snapshot.heroMetricValue)
                    .font(.title2.weight(.semibold))
                Text(snapshot.heroMetricUnit)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            if !snapshot.heroTrendText.isEmpty {
                Text(snapshot.heroTrendText)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
        }
        .widgetAccessibilityLabel(accessibilityValue)
        .accessibilityIdentifier("heroMetricRectangular")
    }

    private var small: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(snapshot.heroMetricName)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(2)
            Spacer(minLength: 0)
            HStack(alignment: .firstTextBaseline, spacing: 4) {
                Text(snapshot.heroMetricValue)
                    .font(.system(size: 40, weight: .bold, design: .rounded))
                    .foregroundStyle(WidgetTheme.terracotta)
                    .minimumScaleFactor(0.5)
                    .lineLimit(1)
                Text(snapshot.heroMetricUnit)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            if !snapshot.heroTrendText.isEmpty {
                Text(snapshot.heroTrendText)
                    .font(.caption2)
                    .foregroundStyle(snapshot.heroTrendIsPositive ? WidgetTheme.sage : .red)
                    .lineLimit(1)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .widgetAccessibilityLabel(accessibilityValue)
        .accessibilityIdentifier("heroMetricSmall")
    }
}
