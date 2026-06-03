// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

/// Brand accent used in the `systemSmall` Hero widget. Lock-screen accessory
/// families are tinted by the system, so this only shows on the home-screen
/// small family.
enum WidgetTheme {
    static let terracotta = Color(red: 194 / 255, green: 101 / 255, blue: 74 / 255)
    static let sage = Color(red: 90 / 255, green: 138 / 255, blue: 90 / 255)
}

extension View {
    /// Sets a VoiceOver label while keeping a stable accessibility identifier
    /// for XCUITest snapshot assertions.
    func widgetAccessibilityLabel(_ label: String) -> some View {
        accessibilityLabel(Text(label))
    }
}
