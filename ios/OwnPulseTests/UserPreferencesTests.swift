// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("UserPreferences", .serialized)
struct UserPreferencesTests {
    /// Builds an isolated UserDefaults suite so tests don't clobber the user's
    /// real defaults and don't leak state across runs.
    private func isolatedDefaults() -> UserDefaults {
        let name = "UserPreferencesTests.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defaults.removePersistentDomain(forName: name)
        return defaults
    }

    @Test("default weight unit is pounds on a fresh install")
    func defaultIsPounds() {
        let defaults = isolatedDefaults()
        UserPreferences.configure(defaults: defaults)
        defer { UserPreferences.resetToStandard() }

        #expect(UserPreferences.weightUnit == .pounds)
    }

    @Test("setting the weight unit persists and round-trips")
    func weightUnitRoundTrip() {
        let defaults = isolatedDefaults()
        UserPreferences.configure(defaults: defaults)
        defer { UserPreferences.resetToStandard() }

        UserPreferences.weightUnit = .kilograms
        #expect(UserPreferences.weightUnit == .kilograms)

        UserPreferences.weightUnit = .pounds
        #expect(UserPreferences.weightUnit == .pounds)
    }

    @Test("raw UserDefaults value stores the enum raw value")
    func defaultsStoresRawValue() {
        let defaults = isolatedDefaults()
        UserPreferences.configure(defaults: defaults)
        defer { UserPreferences.resetToStandard() }

        UserPreferences.weightUnit = .kilograms
        #expect(defaults.string(forKey: "op.weight_unit") == "kg")

        UserPreferences.weightUnit = .pounds
        #expect(defaults.string(forKey: "op.weight_unit") == "lb")
    }

    @Test("falls back to pounds when the stored value is unrecognised")
    func fallbackOnBadValue() {
        let defaults = isolatedDefaults()
        defaults.set("stones", forKey: "op.weight_unit")
        UserPreferences.configure(defaults: defaults)
        defer { UserPreferences.resetToStandard() }

        #expect(UserPreferences.weightUnit == .pounds)
    }
}
