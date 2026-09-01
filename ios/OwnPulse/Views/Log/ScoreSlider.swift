// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

/// The value/track color always comes from the caller's per-dimension
/// `accentColor` (see brand.md's TrendIndicator rule: color never carries
/// good/bad meaning on its own). A prior version derived the track color from
/// the score value itself (red below 1/3, gold below 2/3, sage above) — that
/// implied low scores are "bad" and high scores "good", which doesn't hold
/// for every dimension and isn't this app's call to make.
struct ScoreSlider: View {
    let label: String
    @Binding var value: Int
    var range: ClosedRange<Int> = 1...10
    var accentColor: Color = OPColor.terracotta

    /// "N/max" — matches web's check-in score display. Internal (not
    /// private) so tests can verify the format without ViewInspector.
    var valueLabel: String {
        "\(value)/\(range.upperBound)"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(label)
                    .font(.subheadline)
                    .fontWeight(.medium)

                Spacer()

                Text(valueLabel)
                    .font(.system(.title3, design: .rounded, weight: .bold))
                    .foregroundStyle(accentColor)
                    .contentTransition(.numericText())
            }

            Slider(
                value: Binding<Double>(
                    get: { Double(value) },
                    set: { newVal in
                        let clamped = min(max(Int(newVal.rounded()), range.lowerBound), range.upperBound)
                        if clamped != value {
                            value = clamped
                        }
                    }
                ),
                in: Double(range.lowerBound)...Double(range.upperBound),
                step: 1
            )
            .tint(accentColor)
            .sensoryFeedback(.selection, trigger: value)
            .accessibilityIdentifier("scoreSlider-\(label.lowercased())")
        }
    }
}
