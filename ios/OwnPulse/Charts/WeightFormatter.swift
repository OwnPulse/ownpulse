// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

/// Converts and formats body-mass values for display. The backend always
/// returns kilograms; this utility maps to the user's preferred unit for UI
/// purposes only — underlying chart data is never mutated.
enum WeightFormatter {
    static let kgPerPound: Double = 2.20462

    /// Returns the display value and unit string for a kg measurement.
    static func display(kg: Double, prefs: WeightUnitPreference) -> (value: Double, unit: String) {
        switch prefs {
        case .kilograms:
            return (kg, "kg")
        case .pounds:
            return (kg * kgPerPound, "lb")
        }
    }

    /// Formats "kg" for inline text (e.g. axis tick labels, summary cards).
    /// Uses one decimal place — matches the existing weight chart style.
    static func format(kg: Double, prefs: WeightUnitPreference = UserPreferences.weightUnit) -> String {
        let (value, unit) = display(kg: kg, prefs: prefs)
        return String(format: "%.1f %@", value, unit)
    }

    /// Same but without the unit suffix — for when the unit is shown separately
    /// (e.g., a stats card with a dedicated unit row).
    static func formatValueOnly(kg: Double, prefs: WeightUnitPreference = UserPreferences.weightUnit) -> String {
        let (value, _) = display(kg: kg, prefs: prefs)
        return String(format: "%.1f", value)
    }

    /// Unit string for the current preference. Handy for axis labels.
    static func unitString(prefs: WeightUnitPreference = UserPreferences.weightUnit) -> String {
        display(kg: 0, prefs: prefs).unit
    }
}
