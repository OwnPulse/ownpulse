// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

/// Shared axis configuration constants used across all Explore charts.
///
/// Centralised so tests can assert the invariant — "no `AreaMark`-style zero
/// baseline on the Y axis" — without inspecting SwiftUI view trees.
/// `OverlayChartView`, `SmallMultiplesChartView`, `MetricSparklineChart`, and
/// `WeightChartView` all consume `includesZeroInYAxis` when wiring up
/// `chartYScale(domain: .automatic(includesZero:))`.
enum ChartAxisConfig {
    /// Whether the Y-axis domain should snap to zero. Explicitly `false` so
    /// body mass (e.g. 72 kg) doesn't run 0–150 and heart rate / sleep have
    /// their own fitted scales. Regressing this to `true` brings back the
    /// baseline-collapse bug reported on TestFlight 1.1.0(4).
    static let includesZeroInYAxis: Bool = false
}
