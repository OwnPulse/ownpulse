// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import WidgetKit

/// A one-tap lock-screen shortcut that deep-links into the Log screen with the
/// intervention form pre-selected — the most common "log something now" path.
/// The `Widget` configuration lives in `WidgetConfigurations.swift`; this view
/// is shared with the app target for the DEBUG snapshot harness.
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
