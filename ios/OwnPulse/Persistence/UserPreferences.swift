// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

/// Weight unit for display. Backend always stores kg; this toggles presentation
/// only (axis tick labels, latest-value strings, summary stats).
enum WeightUnitPreference: String, CaseIterable, Sendable {
    case kilograms = "kg"
    case pounds = "lb"

    var displayName: String {
        switch self {
        case .kilograms: return "Kilograms (kg)"
        case .pounds: return "Pounds (lb)"
        }
    }
}

/// User-facing display preferences. Stored in UserDefaults — no health data.
enum UserPreferences {
    private static let weightUnitKey = "op.weight_unit"

    /// Underlying UserDefaults store. Overridable for tests via `configure(_:)`.
    private static let lock = NSLock()
    // TODO(concurrency): revisit when we do the broader app-wide Sendable /
    // actor pass — annotate `UserPreferences` as `@MainActor` (or move it to
    // an actor) and remove the `nonisolated(unsafe)` + manual `NSLock`. Today
    // the storage is guarded by `lock` so races are safe at runtime, but the
    // compiler can't prove it.
    nonisolated(unsafe) private static var _defaults: UserDefaults = .standard

    static var defaults: UserDefaults {
        lock.lock()
        defer { lock.unlock() }
        return _defaults
    }

    /// Point preference storage at a custom UserDefaults — used in tests.
    static func configure(defaults: UserDefaults) {
        lock.lock()
        defer { lock.unlock() }
        _defaults = defaults
    }

    /// Reset to the standard UserDefaults. Tests should call this in teardown.
    static func resetToStandard() {
        lock.lock()
        defer { lock.unlock() }
        _defaults = .standard
    }

    /// User's preferred weight unit. Defaults to pounds on first launch.
    static var weightUnit: WeightUnitPreference {
        get {
            let raw = defaults.string(forKey: weightUnitKey) ?? WeightUnitPreference.pounds.rawValue
            return WeightUnitPreference(rawValue: raw) ?? .pounds
        }
        set {
            defaults.set(newValue.rawValue, forKey: weightUnitKey)
        }
    }
}
