// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

/// Pure presentation helpers for `HealthOverviewView`. Extracted to keep the
/// view thin and the logic unit-testable without a SwiftUI runtime.
enum HealthOverviewPresenter {
    /// Human-readable label for a metric field, with weight-unit suffix when
    /// the metric is `body_mass`. Matches the legend entry text.
    static func humanLabel(
        for field: String,
        prefs: WeightUnitPreference = UserPreferences.weightUnit
    ) -> String {
        let base = field.replacingOccurrences(of: "_", with: " ").capitalized
        if field == "body_mass" {
            return "\(base) (\(WeightFormatter.unitString(prefs: prefs)))"
        }
        return base
    }

    /// Sorted, de-duplicated list of substance names from the intervention
    /// markers. Drives the filter pill row.
    static func uniqueSubstances(from interventions: [InterventionMarker]) -> [String] {
        Array(Set(interventions.map(\.substance))).sorted()
    }

    /// Toggle a substance into/out of the hidden-set and return the new set.
    /// Returning rather than mutating makes the logic trivially unit-testable
    /// and lets the view's `@State` drive the UI via plain assignment.
    static func toggleHidden(_ substance: String, in set: Set<String>) -> Set<String> {
        var next = set
        if next.contains(substance) {
            next.remove(substance)
        } else {
            next.insert(substance)
        }
        return next
    }
}
