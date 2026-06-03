// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

/// User's explicit light/dark/system appearance preference. Persisted via
/// `@AppStorage("preferredColorScheme")` and applied at the root scene with
/// `.preferredColorScheme(_:)`. Mirrors the web tri-state in `useTheme`.
///
/// Stored as a raw `String` in UserDefaults so `@AppStorage` round-trips it
/// directly. `.system` defers to the OS appearance.
enum ColorSchemePreference: String, CaseIterable, Sendable {
    case system
    case light
    case dark

    /// The `@AppStorage` key. Kept here so the view and tests share one source
    /// of truth.
    static let storageKey = "preferredColorScheme"

    /// The SwiftUI `ColorScheme` to force, or `nil` to follow the system.
    var colorScheme: ColorScheme? {
        switch self {
        case .system: return nil
        case .light: return .light
        case .dark: return .dark
        }
    }

    var displayName: String {
        switch self {
        case .system: return "System"
        case .light: return "Light"
        case .dark: return "Dark"
        }
    }

    /// Maps an arbitrary stored raw value to a preference, defaulting to
    /// `.system` for missing or unrecognised values.
    static func from(rawValue: String?) -> ColorSchemePreference {
        guard let rawValue, let pref = ColorSchemePreference(rawValue: rawValue) else {
            return .system
        }
        return pref
    }
}
