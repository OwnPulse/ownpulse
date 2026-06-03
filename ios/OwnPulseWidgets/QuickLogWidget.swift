// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import WidgetKit

/// A one-tap lock-screen shortcut that deep-links into the Log screen with the
/// intervention form pre-selected — the most common "log something now" path.
struct QuickLogWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: WidgetSharedConstants.quickLogKind, provider: OwnPulseProvider()) { entry in
            QuickLogWidgetView(entry: entry)
                .widgetURL(URL(string: "ownpulse://log?form=intervention"))
        }
        .configurationDisplayName("Quick Log")
        .description("Jump straight to logging an intervention.")
        .supportedFamilies([.accessoryCircular])
    }
}

struct QuickLogWidgetView: View {
    let entry: OwnPulseEntry

    var body: some View {
        ZStack {
            AccessoryWidgetBackground()
            Image(systemName: "plus.circle.fill")
                .font(.title)
                .accessibilityIdentifier("quickLogGlyph")
        }
        .widgetAccessibilityLabel("Quick log an intervention")
        .accessibilityIdentifier("quickLogCircular")
    }
}
