// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

/// Tests the data path the Settings > Units > Weight picker drives: its
/// `onChange` assigns the new `WeightUnitPreference` to
/// `UserPreferences.weightUnit`. We don't need SwiftUI here — asserting the
/// assignment round-trips through the defaults store is sufficient, and the
/// Maestro flow `weight-unit-preference.yaml` covers the rendered picker.
@Suite("Settings Units Picker", .serialized)
struct SettingsUnitsPickerTests {
    private func isolatedDefaults() -> UserDefaults {
        let name = "SettingsUnitsPickerTests.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defaults.removePersistentDomain(forName: name)
        return defaults
    }

    @Test("picker write of .kilograms persists through UserPreferences")
    func writeKilograms() {
        let defaults = isolatedDefaults()
        UserPreferences.configure(defaults: defaults)
        defer { UserPreferences.resetToStandard() }

        // Simulates the picker's `onChange` body.
        UserPreferences.weightUnit = .kilograms

        // A fresh read must observe the new value (mirror of SettingsView's
        // @State initializer reading UserPreferences.weightUnit at appear).
        #expect(UserPreferences.weightUnit == .kilograms)
        #expect(defaults.string(forKey: "op.weight_unit") == "kg")
    }

    @Test("picker write of .pounds persists through UserPreferences")
    func writePounds() {
        let defaults = isolatedDefaults()
        UserPreferences.configure(defaults: defaults)
        defer { UserPreferences.resetToStandard() }

        UserPreferences.weightUnit = .pounds

        #expect(UserPreferences.weightUnit == .pounds)
        #expect(defaults.string(forKey: "op.weight_unit") == "lb")
    }

    @Test("toggling the picker back-and-forth preserves the final value")
    func toggleBackAndForth() {
        let defaults = isolatedDefaults()
        UserPreferences.configure(defaults: defaults)
        defer { UserPreferences.resetToStandard() }

        UserPreferences.weightUnit = .kilograms
        UserPreferences.weightUnit = .pounds
        UserPreferences.weightUnit = .kilograms

        #expect(UserPreferences.weightUnit == .kilograms)
    }
}
