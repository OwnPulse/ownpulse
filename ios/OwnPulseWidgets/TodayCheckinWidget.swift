// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import WidgetKit

/// Lock-screen widget view showing whether today's subjective check-in is
/// filled. The `Widget` configuration that wires this up to the timeline
/// provider lives in `WidgetConfigurations.swift` (extension-only); this view
/// is shared with the app target so the DEBUG snapshot harness can render it.
struct TodayCheckinWidgetView: View {
    // `\.widgetFamily` is a read-only environment key (WidgetKit sets it), so
    // it can't be injected via `.environment(...)`. For the DEBUG snapshot
    // harness we accept an explicit override; in production it stays nil and
    // we read the real family WidgetKit provides.
    @Environment(\.widgetFamily) private var environmentFamily
    let entry: OwnPulseEntry
    var familyOverride: WidgetFamily?

    private var family: WidgetFamily { familyOverride ?? environmentFamily }
    private var filled: Bool { entry.snapshot.checkinFilledToday }

    var body: some View {
        switch family {
        case .accessoryCircular:
            circular
        default:
            rectangular
        }
    }

    private var circular: some View {
        ZStack {
            AccessoryWidgetBackground()
            Image(systemName: filled ? "checkmark.circle.fill" : "circle.dashed")
                .font(.title2)
                .accessibilityIdentifier("checkinCircularGlyph")
        }
        .widgetAccessibilityLabel(filled ? "Check-in done" : "Check-in pending")
        .accessibilityIdentifier("todayCheckinCircular")
    }

    private var rectangular: some View {
        HStack(spacing: 8) {
            Image(systemName: filled ? "checkmark.circle.fill" : "circle.dashed")
                .font(.title3)
            VStack(alignment: .leading, spacing: 1) {
                Text("Check-in")
                    .font(.headline)
                Text(filled ? "Done for today" : "Not logged yet")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer(minLength: 0)
        }
        .widgetAccessibilityLabel(filled ? "Check-in done for today" : "Check-in not logged yet")
        .accessibilityIdentifier("todayCheckinRectangular")
    }
}
