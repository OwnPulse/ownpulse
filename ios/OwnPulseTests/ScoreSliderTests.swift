// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import Testing

@testable import OwnPulse

/// Guards against ScoreSlider regressing to a value-derived track color (see
/// the doc comment on the struct) and pins the "N/max" value format.
@MainActor
struct ScoreSliderTests {

    @Test("accentColor is the caller-supplied color regardless of value", arguments: [
        (1, OPColor.dimensionEnergy),
        (5, OPColor.terracotta),
        (10, OPColor.sage),
    ])
    func accentColorPassesThroughUnchanged(value: Int, color: Color) {
        var boundValue = value
        let binding = Binding(get: { boundValue }, set: { boundValue = $0 })
        let slider = ScoreSlider(label: "Test", value: binding, accentColor: color)
        #expect(slider.accentColor == color)
    }

    @Test("valueLabel renders as N/max", arguments: [
        (1, 1...10, "1/10"),
        (10, 1...10, "10/10"),
        (5, 0...5, "5/5"),
    ])
    func valueLabelFormat(value: Int, range: ClosedRange<Int>, expected: String) {
        var boundValue = value
        let binding = Binding(get: { boundValue }, set: { boundValue = $0 })
        let slider = ScoreSlider(label: "Test", value: binding, range: range)
        #expect(slider.valueLabel == expected)
    }
}
