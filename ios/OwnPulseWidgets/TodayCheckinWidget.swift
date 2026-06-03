// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import WidgetKit

/// Lock-screen widget showing whether today's subjective check-in is filled.
/// Tapping deep-links into the check-in form.
struct TodayCheckinWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: WidgetSharedConstants.todayCheckinKind, provider: OwnPulseProvider()) { entry in
            TodayCheckinWidgetView(entry: entry)
                .widgetURL(URL(string: "ownpulse://log?form=checkin"))
        }
        .configurationDisplayName("Today's Check-in")
        .description("See at a glance whether you've logged today's check-in.")
        .supportedFamilies([.accessoryCircular, .accessoryRectangular])
    }
}

struct TodayCheckinWidgetView: View {
    @Environment(\.widgetFamily) private var family
    let entry: OwnPulseEntry

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
