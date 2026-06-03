// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

/// Direction of a metric trend, with a colorblind-safe presentation.
///
/// Trend direction is conveyed by BOTH a color and a directional arrow, never
/// color alone. The arrow guarantees the direction reads in grayscale and for
/// users with any form of color vision deficiency — the shape (up-right vs
/// down-right vs flat) is the primary signal; color is secondary reinforcement.
///
/// Colors come from the Wong colorblind-safe palette, sourced from the
/// generated `ChartColors` tokens (shared with the web frontend) so we don't
/// reintroduce ad-hoc literals:
/// - up:   `#d55e00` (Wong vermillion) — the `heart_rate` token
/// - down: `#0072B2` (Wong blue)       — the `glucose` token
///
/// This replaces the previous `OPColor.trendUp`/`trendDown` red/green scheme,
/// which was a classic red-green colorblind failure and was also semantically
/// inverted (up was red, down was green).
enum TrendDirection {
    case up
    case down
    case flat

    /// SF Symbol whose shape encodes the direction independently of color.
    var systemImage: String {
        switch self {
        case .up: return "arrow.up.right"
        case .down: return "arrow.down.right"
        case .flat: return "arrow.forward"
        }
    }

    /// Wong colorblind-safe color for the direction.
    var color: Color {
        switch self {
        case .up: return ChartColors.metric["heart_rate"] ?? OPColor.trendUp
        case .down: return ChartColors.metric["glucose"] ?? OPColor.trendDown
        case .flat: return OPColor.trendFlat
        }
    }

    /// VoiceOver phrase describing the direction, so non-visual users get the
    /// same information the arrow and color convey.
    var spokenDescription: String {
        switch self {
        case .up: return "trending up"
        case .down: return "trending down"
        case .flat: return "holding steady"
        }
    }
}
