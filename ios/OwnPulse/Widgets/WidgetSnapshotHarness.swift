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
        // Resting HR dropping 4% is "good" (isPositive) but the data went DOWN
        // — the arrow must follow the data, not the polarity.
        heroTrendIsPositive: true,
        heroTrendDirection: .down,
        lastUpdated: Date(timeIntervalSince1970: 1_700_000_000)
    )

    private var entry: OwnPulseEntry {
        OwnPulseEntry(date: Date(timeIntervalSince1970: 1_700_000_000), snapshot: Self.sampleSnapshot)
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 24) {
                // Always-present marker the XCUITest waits on to confirm the
                // harness rendered. A plain Text surfaces reliably in the
                // accessibility tree (unlike a ScrollView container id).
                Text("Widget Snapshot Harness")
                    .font(.footnote)
                    .accessibilityIdentifier("widgetSnapshotHarness")

                // Render each widget view at every supported family. We do NOT
                // wrap the views in a container that carries an accessibility
                // identifier — SwiftUI would merge the children's identifiers
                // away, hiding the per-widget glyphs the test asserts on.
                labelled("Today Check-in — Circular") {
                    TodayCheckinWidgetView(entry: entry, familyOverride: .accessoryCircular)
                }
                labelled("Today Check-in — Rectangular") {
                    TodayCheckinWidgetView(entry: entry, familyOverride: .accessoryRectangular)
                }
                labelled("Hero Metric — Rectangular") {
                    HeroMetricWidgetView(entry: entry, familyOverride: .accessoryRectangular)
                }
                labelled("Hero Metric — Small") {
                    HeroMetricWidgetView(entry: entry, familyOverride: .systemSmall)
                        .frame(width: 158, height: 158)
                }
                labelled("Quick Log — Circular") {
                    QuickLogWidgetView(entry: entry)
                }
            }
            .padding()
        }
    }

    /// Lays out a caption above the widget view WITHOUT wrapping the view in an
    /// identifier-bearing container, so the widget's own accessibility
    /// identifiers stay queryable by the XCUITest.
    @ViewBuilder
    private func labelled(_ title: String, @ViewBuilder content: () -> some View) -> some View {
        Text(title)
            .font(.footnote)
            .foregroundStyle(.secondary)
        content()
            .frame(width: 140, height: 140)
            .background(Color.gray.opacity(0.2))
    }
}
#endif
