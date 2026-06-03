// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

#if DEBUG
import SwiftUI
import WidgetKit

/// A DEBUG-only screen that renders every widget view at every supported
/// family inside `WidgetPreviewContext`, so an XCUITest can assert that all
/// three families render their expected content. It is NOT part of the
/// production UI — `OwnPulseApp` only shows it when launched with the
/// `-WidgetSnapshotHarness` argument.
///
/// This is the closest CI-friendly stand-in for a real Home Screen / Lock
/// Screen render: `WidgetPreviewContext` drives the exact same SwiftUI the
/// extension ships, but inside the host app we can drive via accessibility.
struct WidgetSnapshotHarness: View {
    /// Fixed, deterministic snapshot so assertions are stable.
    static let sampleSnapshot = WidgetSnapshot(
        checkinFilledToday: true,
        heroMetricName: "Resting Heart Rate",
        heroMetricValue: "56",
        heroMetricUnit: "bpm",
        heroTrendText: "-4% vs 30d avg",
        heroTrendIsPositive: true,
        lastUpdated: Date(timeIntervalSince1970: 1_700_000_000)
    )

    private var entry: OwnPulseEntry {
        OwnPulseEntry(date: Date(timeIntervalSince1970: 1_700_000_000), snapshot: Self.sampleSnapshot)
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 24) {
                section("Today Check-in — Circular", id: "harnessCheckinCircular") {
                    TodayCheckinWidgetView(entry: entry, familyOverride: .accessoryCircular)
                }
                section("Today Check-in — Rectangular", id: "harnessCheckinRectangular") {
                    TodayCheckinWidgetView(entry: entry, familyOverride: .accessoryRectangular)
                }
                section("Hero Metric — Rectangular", id: "harnessHeroRectangular") {
                    HeroMetricWidgetView(entry: entry, familyOverride: .accessoryRectangular)
                }
                section("Hero Metric — Small", id: "harnessHeroSmall") {
                    HeroMetricWidgetView(entry: entry, familyOverride: .systemSmall)
                        .frame(width: 158, height: 158)
                }
                section("Quick Log — Circular", id: "harnessQuickLogCircular") {
                    QuickLogWidgetView(entry: entry)
                }
            }
            .padding()
        }
        .accessibilityIdentifier("widgetSnapshotHarness")
    }

    @ViewBuilder
    private func section(_ title: String, id: String, @ViewBuilder content: () -> some View) -> some View {
        VStack(spacing: 8) {
            Text(title)
                .font(.footnote)
                .foregroundStyle(.secondary)
            content()
                .frame(width: 120, height: 120)
                .background(Color.gray.opacity(0.2))
        }
        .accessibilityIdentifier(id)
    }
}
#endif
