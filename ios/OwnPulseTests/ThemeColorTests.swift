// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import Testing
import UIKit

@testable import OwnPulse

/// Verifies the muted-text token resolves to values that clear WCAG AA
/// contrast, so the login-screen accessibility audit cannot silently regress
/// back to a "Contrast nearly passed" failure.
@MainActor
struct ThemeColorTests {

    /// WCAG relative-luminance contrast ratio between two opaque colors.
    private func contrastRatio(_ a: UIColor, _ b: UIColor) -> Double {
        func luminance(_ color: UIColor) -> Double {
            var r: CGFloat = 0, g: CGFloat = 0, bl: CGFloat = 0, alpha: CGFloat = 0
            color.getRed(&r, green: &g, blue: &bl, alpha: &alpha)
            func channel(_ c: CGFloat) -> Double {
                let v = Double(c)
                return v <= 0.03928 ? v / 12.92 : pow((v + 0.055) / 1.055, 2.4)
            }
            return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(bl)
        }
        let l1 = luminance(a)
        let l2 = luminance(b)
        let hi = max(l1, l2)
        let lo = min(l1, l2)
        return (hi + 0.05) / (lo + 0.05)
    }

    private func resolved(_ color: Color, style: UIUserInterfaceStyle) -> UIColor {
        UIColor(color).resolvedColor(with: UITraitCollection(userInterfaceStyle: style))
    }

    @Test("mutedText clears WCAG AA (>= 4.5:1) on the light surface background")
    func mutedTextPassesContrastInLightMode() {
        let fg = resolved(OPColor.mutedText, style: .light)
        let bg = resolved(OPColor.warmBg, style: .light)
        let ratio = contrastRatio(fg, bg)
        #expect(ratio >= 4.5, "mutedText on warmBg was \(ratio):1, below WCAG AA 4.5:1")
    }

    @Test("mutedText clears WCAG AA (>= 4.5:1) on the dark background")
    func mutedTextPassesContrastInDarkMode() {
        let fg = resolved(OPColor.mutedText, style: .dark)
        let bg = resolved(OPColor.darkBg, style: .dark)
        let ratio = contrastRatio(fg, bg)
        #expect(ratio >= 4.5, "mutedText on darkBg was \(ratio):1, below WCAG AA 4.5:1")
    }

    @Test("mutedText resolves to distinct light and dark values")
    func mutedTextIsAdaptive() {
        let light = resolved(OPColor.mutedText, style: .light)
        let dark = resolved(OPColor.mutedText, style: .dark)
        #expect(light != dark, "mutedText should adapt between light and dark mode")
    }
}
