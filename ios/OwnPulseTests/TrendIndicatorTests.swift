// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import Testing
import UIKit

@testable import OwnPulse

/// Verifies the colorblind-safe trend mapping: each `TrendDirection` resolves
/// to (1) the correct Wong palette color and (2) a distinct directional arrow,
/// so trend direction is never conveyed by color alone.
///
/// Grayscale-distinguishability note: the three directions map to three
/// distinct SF Symbols (`arrow.up.right`, `arrow.down.right`, `arrow.forward`).
/// The arrow shape is the primary signal and is fully distinguishable with no
/// color information at all — these tests assert the symbols differ so a
/// regression that collapses them (reintroducing color-only signalling) fails.
@MainActor
struct TrendIndicatorTests {

    private func rgb(_ color: Color) -> (r: Int, g: Int, b: Int) {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        UIColor(color).getRed(&r, green: &g, blue: &b, alpha: &a)
        return (Int((r * 255).rounded()), Int((g * 255).rounded()), Int((b * 255).rounded()))
    }

    @Test("up maps to the up-right arrow")
    func upArrow() {
        #expect(TrendDirection.up.systemImage == "arrow.up.right")
    }

    @Test("down maps to the down-right arrow")
    func downArrow() {
        #expect(TrendDirection.down.systemImage == "arrow.down.right")
    }

    @Test("flat maps to the forward arrow")
    func flatArrow() {
        #expect(TrendDirection.flat.systemImage == "arrow.forward")
    }

    @Test("all three directions use distinct arrow shapes (grayscale-distinguishable)")
    func arrowsAreDistinct() {
        let symbols = Set([
            TrendDirection.up.systemImage,
            TrendDirection.down.systemImage,
            TrendDirection.flat.systemImage,
        ])
        #expect(symbols.count == 3, "trend directions must be distinguishable by shape, not color alone")
    }

    @Test("up uses Wong vermillion #d55e00")
    func upColorIsWongVermillion() {
        let c = rgb(TrendDirection.up.color)
        #expect(c == (213, 94, 0), "up color was \(c), expected Wong vermillion (213, 94, 0)")
    }

    @Test("down uses Wong blue #0072B2")
    func downColorIsWongBlue() {
        let c = rgb(TrendDirection.down.color)
        #expect(c == (0, 114, 178), "down color was \(c), expected Wong blue (0, 114, 178)")
    }

    @Test("up and down colors are not the legacy red/green and differ from each other")
    func colorsAreNotRedGreen() {
        let up = rgb(TrendDirection.up.color)
        let down = rgb(TrendDirection.down.color)
        #expect(up != down, "up and down must be different colors")
        // Pure red and pure green were the old, colorblind-unsafe values.
        #expect(up != (255, 0, 0), "up must not be pure red (legacy)")
        #expect(down != (0, 255, 0), "down must not be pure green (legacy)")
    }

    @Test("trend colors are sourced from the generated ChartColors tokens")
    func colorsMatchTokens() {
        #expect(rgb(TrendDirection.up.color) == rgb(ChartColors.metric["heart_rate"]!))
        #expect(rgb(TrendDirection.down.color) == rgb(ChartColors.metric["glucose"]!))
    }

    @Test("OPColor trend members agree with TrendDirection (single source of truth)")
    func opColorTrendMatchesDirection() {
        #expect(rgb(OPColor.trendUp) == rgb(TrendDirection.up.color))
        #expect(rgb(OPColor.trendDown) == rgb(TrendDirection.down.color))
    }

    @Test("spoken descriptions are distinct and direction-appropriate")
    func spokenDescriptions() {
        #expect(TrendDirection.up.spokenDescription == "trending up")
        #expect(TrendDirection.down.spokenDescription == "trending down")
        #expect(TrendDirection.flat.spokenDescription == "holding steady")
    }
}
