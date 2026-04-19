// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("WeightFormatter")
struct WeightFormatterTests {
    @Test("kilograms preference returns input value and kg unit")
    func kilogramsIdentity() {
        let (value, unit) = WeightFormatter.display(kg: 72.5, prefs: .kilograms)
        #expect(value == 72.5)
        #expect(unit == "kg")
    }

    @Test("zero kg maps to zero in both units")
    func zeroRoundTrip() {
        let (kgValue, kgUnit) = WeightFormatter.display(kg: 0, prefs: .kilograms)
        #expect(kgValue == 0)
        #expect(kgUnit == "kg")

        let (lbValue, lbUnit) = WeightFormatter.display(kg: 0, prefs: .pounds)
        #expect(lbValue == 0)
        #expect(lbUnit == "lb")
    }

    @Test("pounds preference converts kg to lb using 2.20462")
    func poundsConversion() {
        let (value, unit) = WeightFormatter.display(kg: 70, prefs: .pounds)
        // 70 * 2.20462 = 154.3234
        #expect(abs(value - 154.3234) < 0.0005)
        #expect(unit == "lb")
    }

    @Test("format(kg:prefs:) renders one decimal with unit suffix")
    func formatOneDecimal() {
        #expect(WeightFormatter.format(kg: 70, prefs: .pounds) == "154.3 lb")
        #expect(WeightFormatter.format(kg: 72.5, prefs: .kilograms) == "72.5 kg")
    }

    @Test("formatValueOnly(kg:prefs:) omits the unit suffix")
    func formatValueOnlyOmitsUnit() {
        #expect(WeightFormatter.formatValueOnly(kg: 70, prefs: .pounds) == "154.3")
        #expect(WeightFormatter.formatValueOnly(kg: 72.5, prefs: .kilograms) == "72.5")
    }

    @Test("unitString returns the preference's display unit")
    func unitString() {
        #expect(WeightFormatter.unitString(prefs: .kilograms) == "kg")
        #expect(WeightFormatter.unitString(prefs: .pounds) == "lb")
    }

    @Test("negative kg (delta) converts correctly")
    func negativeDelta() {
        let (value, unit) = WeightFormatter.display(kg: -1.5, prefs: .pounds)
        #expect(abs(value - (-3.30693)) < 0.0005)
        #expect(unit == "lb")
    }
}
