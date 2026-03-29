// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct ScoreSlider: View {
    let label: String
    @Binding var value: Int
    var range: ClosedRange<Int> = 1...10
    var accentColor: Color = OPColor.terracotta

    private var normalizedProgress: Double {
        let rangeSize = Double(range.upperBound - range.lowerBound)
        guard rangeSize > 0 else { return 0 }
        return Double(value - range.lowerBound) / rangeSize
    }

    private var trackColor: Color {
        let t = normalizedProgress
        if t < 0.33 {
            return Color.red.opacity(0.6)
        } else if t < 0.66 {
            return OPColor.gold.opacity(0.7)
        } else {
            return OPColor.sage.opacity(0.7)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(label)
                    .font(.subheadline)
                    .fontWeight(.medium)

                Spacer()

                Text("\(value)")
                    .font(.system(.title3, design: .rounded, weight: .bold))
                    .foregroundStyle(trackColor)
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
            .tint(trackColor)
            .sensoryFeedback(.selection, trigger: value)
            .accessibilityIdentifier("scoreSlider-\(label.lowercased())")
        }
    }
}
