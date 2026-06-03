// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

enum OPColor {
    static let terracotta = Color(red: 194 / 255, green: 101 / 255, blue: 74 / 255)
    static let teal = Color(red: 61 / 255, green: 139 / 255, blue: 139 / 255)
    static let gold = Color(red: 196 / 255, green: 154 / 255, blue: 60 / 255)
    static let sage = Color(red: 90 / 255, green: 138 / 255, blue: 90 / 255)
    static let warmBg = Color(red: 250 / 255, green: 246 / 255, blue: 241 / 255)
    static let darkBg = Color(red: 26 / 255, green: 26 / 255, blue: 26 / 255)
    static let cardDark = Color(red: 34 / 255, green: 34 / 255, blue: 34 / 255)
    static let cardLight = Color.white

    /// Muted secondary text that still clears WCAG AA (4.5:1 for normal text).
    ///
    /// SwiftUI's `.secondary` foreground style is translucent and composites to
    /// a ratio that dips just under 4.5:1 on a light background — the iOS
    /// accessibility audit flags it as "Contrast nearly passed". This token is
    /// an opaque value from the brand neutral scale so the ratio is
    /// deterministic: neutral-600 (#5e5e57) on the light surface is ~6.3:1, and
    /// neutral-300 (#c2c2b9) on the dark background is ~9.7:1. Use this for
    /// secondary/caption text instead of `.secondary` where the audit applies.
    static let mutedText = Color(
        light: Color(red: 94 / 255, green: 94 / 255, blue: 87 / 255),
        dark: Color(red: 194 / 255, green: 194 / 255, blue: 185 / 255)
    )

    static let trendUp = Color.red
    static let trendDown = Color.green
    static let trendFlat = Color.secondary
}

extension Color {
    /// Builds a color that resolves to `light` in light mode and `dark` in
    /// dark mode, via a `UIColor` dynamic provider.
    init(light: Color, dark: Color) {
        self = Color(uiColor: UIColor { traits in
            UIColor(traits.userInterfaceStyle == .dark ? dark : light)
        })
    }
}

struct OPCardModifier: ViewModifier {
    @Environment(\.colorScheme) private var colorScheme

    func body(content: Content) -> some View {
        content
            .padding(16)
            .background(
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .fill(colorScheme == .dark ? .ultraThinMaterial : .regularMaterial)
            )
            .shadow(color: .black.opacity(colorScheme == .dark ? 0.3 : 0.08), radius: 8, y: 4)
    }
}

extension View {
    func opCard() -> some View {
        modifier(OPCardModifier())
    }
}
