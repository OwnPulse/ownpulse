// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

/// Pure presenter logic for `MetricBrowseCard`. Extracted so the display-unit
/// and latest-value formatting can be unit-tested without a SwiftUI runtime.
///
/// The card itself delegates to these functions; adding tests at this layer
/// is the simplest way to guard against regressions (e.g. body_mass losing
/// its `WeightFormatter` hop) without adding a ViewInspector dependency.
enum BrowseCardPresenter {
    /// The unit string to show under the metric label — for body_mass this
    /// follows the user's weight-unit preference, everything else uses the
    /// backend-supplied unit.
    static func displayUnit(
        field: String,
        unit: String,
        prefs: WeightUnitPreference = UserPreferences.weightUnit
    ) -> String {
        if field == "body_mass" {
            return WeightFormatter.unitString(prefs: prefs)
        }
        return unit
    }

    /// Visual state enum for the sparkline slot on the card. Tests can
    /// exercise this independently of SwiftUI view identity.
    enum SparklineState {
        case chart(points: [DataPoint])
        case loading
        case empty
    }

    static func sparklineState(points: [DataPoint]?, isLoading: Bool) -> SparklineState {
        if let points, !points.isEmpty {
            return .chart(points: points)
        }
        if isLoading {
            return .loading
        }
        return .empty
    }

    /// The latest-value line under the sparkline. Returns `nil` if there's
    /// nothing to show (caller renders a placeholder "—" in that case).
    static func latestValueText(
        field: String,
        points: [DataPoint]?,
        prefs: WeightUnitPreference = UserPreferences.weightUnit
    ) -> String? {
        guard let last = points?.last else { return nil }
        if field == "body_mass" {
            return WeightFormatter.formatValueOnly(kg: last.v, prefs: prefs)
        }
        // Show a compact number: 1 decimal if the value is small enough to
        // warrant precision, otherwise no decimals.
        if abs(last.v) < 10 {
            return String(format: "%.1f", last.v)
        }
        return String(format: "%.0f", last.v)
    }
}

extension BrowseCardPresenter.SparklineState {
    static func == (
        lhs: BrowseCardPresenter.SparklineState,
        rhs: BrowseCardPresenter.SparklineState
    ) -> Bool {
        switch (lhs, rhs) {
        case (.chart(let a), .chart(let b)):
            return a.count == b.count && zip(a, b).allSatisfy { $0.t == $1.t && $0.v == $1.v }
        case (.loading, .loading):
            return true
        case (.empty, .empty):
            return true
        default:
            return false
        }
    }
}
