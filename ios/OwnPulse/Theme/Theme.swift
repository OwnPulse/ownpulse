// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

// The brand palette (terracotta, teal, gold, sage, warmBg, cardLight) is
// generated from docs/design/tokens.json into Tokens.swift. The members below
// are hand-written: they are either not yet modeled in the token source
// (darkBg, cardDark), deliberately tuned for WCAG AA contrast in a way the raw
// palette does not express (mutedText, googleBlue), or colorblind-safe trend
// indicators sourced from the generated ChartColors Wong palette (always
// paired with a directional arrow — see TrendDirection).
extension OPColor {
    static let darkBg = Color(red: 26 / 255, green: 26 / 255, blue: 26 / 255)
    static let cardDark = Color(red: 34 / 255, green: 34 / 255, blue: 34 / 255)

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

    /// Blue for the "Sign in with Google" button background.
    ///
    /// The system `.blue` against white label text composites to ~4.0:1, below
    /// WCAG AA (4.5:1) — the accessibility audit flags it "Contrast nearly
    /// passed". This darker blue (#1a5fb4) reads the same but gives ~6.3:1 with
    /// white text, clearing AA.
    static let googleBlue = Color(red: 26 / 255, green: 95 / 255, blue: 180 / 255)

    // Colorblind-safe trend colors from the Wong palette, sourced from the
    // generated ChartColors tokens. These are always paired with a directional
    // arrow wherever they appear (see TrendDirection) — direction is never
    // conveyed by color alone. Replaces the former red(up)/green(down) scheme,
    // which failed red-green color vision and was semantically inverted.
    static let trendUp = ChartColors.metric["heart_rate"] ?? Color(red: 213 / 255, green: 94 / 255, blue: 0 / 255)
    static let trendDown = ChartColors.metric["glucose"] ?? Color(red: 0 / 255, green: 114 / 255, blue: 178 / 255)
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
