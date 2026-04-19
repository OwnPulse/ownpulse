// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

/// Guards the "no zero baseline" invariant used by every Explore chart.
/// `OverlayChartView`, `SmallMultiplesChartView`, `MetricSparklineChart`,
/// and `WeightChartView` all consume `ChartAxisConfig.includesZeroInYAxis`
/// when wiring up `.chartYScale(domain: .automatic(includesZero:))`. Flipping
/// this to `true` brings back the body-mass 0-150 kg y-axis bug reported on
/// TestFlight 1.1.0(4).
@Suite("ChartAxisConfig")
struct ChartAxisConfigTests {
    @Test("includesZeroInYAxis stays false — regression would re-introduce the 0-150 kg bug")
    func includesZeroStaysFalse() {
        #expect(ChartAxisConfig.includesZeroInYAxis == false)
    }
}

/// Guards that body-mass axis tick labels render with `WeightFormatter` —
/// both `OverlayChartView` (when its metric list contains `body_mass`) and
/// `SmallMultiplesChartView` (per-panel for `body_mass`) use the same code
/// path: call `WeightFormatter.formatValueOnly(kg:prefs:)` and render the
/// resulting string. Regressing body_mass back to raw kg (e.g. by dropping
/// the `if isBodyMass`/`if isBodyMassAxis` branch) would leave the axis
/// reading "72.5" with no unit semantics; this test locks in the transform.
@Suite("Axis label transform")
struct AxisLabelTransformTests {
    @Test("body_mass axis tick 70 kg formats to 154.3 when pref is pounds")
    func bodyMassTickPounds() {
        let label = WeightFormatter.formatValueOnly(kg: 70, prefs: .pounds)
        #expect(label == "154.3")
    }

    @Test("body_mass axis tick 70 kg formats to 70.0 when pref is kilograms")
    func bodyMassTickKilograms() {
        let label = WeightFormatter.formatValueOnly(kg: 70, prefs: .kilograms)
        #expect(label == "70.0")
    }
}
