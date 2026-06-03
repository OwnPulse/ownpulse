// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import Testing
@testable import OwnPulse

/// Covers the C12 light/dark/system toggle's data path: mapping the raw
/// `@AppStorage("preferredColorScheme")` string to a `ColorSchemePreference`
/// and on to the SwiftUI `ColorScheme?` applied at the root scene. The
/// rendered Picker is covered by the Maestro flow `theme-toggle.yaml`.
@Suite("ColorSchemePreference")
struct ColorSchemePreferenceTests {
    @Test("each preference maps to the expected ColorScheme")
    func mapsToColorScheme() {
        #expect(ColorSchemePreference.system.colorScheme == nil)
        #expect(ColorSchemePreference.light.colorScheme == .light)
        #expect(ColorSchemePreference.dark.colorScheme == .dark)
    }

    @Test("raw values round-trip through the enum")
    func rawValueRoundTrip() {
        #expect(ColorSchemePreference(rawValue: "system") == .system)
        #expect(ColorSchemePreference(rawValue: "light") == .light)
        #expect(ColorSchemePreference(rawValue: "dark") == .dark)
    }

    @Test("from(rawValue:) returns the matching preference")
    func fromKnownRawValue() {
        #expect(ColorSchemePreference.from(rawValue: "light") == .light)
        #expect(ColorSchemePreference.from(rawValue: "dark") == .dark)
        #expect(ColorSchemePreference.from(rawValue: "system") == .system)
    }

    @Test("from(rawValue:) defaults to system for missing or unknown values")
    func fromBadRawValueDefaultsToSystem() {
        #expect(ColorSchemePreference.from(rawValue: nil) == .system)
        #expect(ColorSchemePreference.from(rawValue: "") == .system)
        #expect(ColorSchemePreference.from(rawValue: "midnight") == .system)
    }

    @Test("AppStorage round-trips the preference and survives a fresh read")
    func appStoragePersistenceRoundTrip() {
        // Mirrors how SettingsView's Picker writes the raw value and how the
        // root scene reads it back on relaunch — a fresh UserDefaults read.
        let name = "ColorSchemePreferenceTests.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defaults.removePersistentDomain(forName: name)
        defer { defaults.removePersistentDomain(forName: name) }

        defaults.set(ColorSchemePreference.dark.rawValue, forKey: ColorSchemePreference.storageKey)

        let stored = defaults.string(forKey: ColorSchemePreference.storageKey)
        #expect(ColorSchemePreference.from(rawValue: stored) == .dark)
        #expect(ColorSchemePreference.from(rawValue: stored).colorScheme == .dark)
    }

    @Test("all three cases are exposed for the picker")
    func allCasesPresent() {
        #expect(ColorSchemePreference.allCases == [.system, .light, .dark])
    }
}
