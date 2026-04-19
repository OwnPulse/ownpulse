// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("BrowseCardPresenter")
struct BrowseCardPresenterTests {
    // MARK: - displayUnit

    @Test("body_mass uses WeightFormatter unit when pref is pounds")
    func displayUnitBodyMassPounds() {
        let unit = BrowseCardPresenter.displayUnit(
            field: "body_mass",
            unit: "kg",
            prefs: .pounds
        )
        #expect(unit == "lb")
    }

    @Test("body_mass uses WeightFormatter unit when pref is kilograms")
    func displayUnitBodyMassKilograms() {
        let unit = BrowseCardPresenter.displayUnit(
            field: "body_mass",
            unit: "kg",
            prefs: .kilograms
        )
        #expect(unit == "kg")
    }

    @Test("non-body_mass fields pass through the backend-supplied unit")
    func displayUnitPassthrough() {
        #expect(
            BrowseCardPresenter.displayUnit(field: "heart_rate", unit: "bpm", prefs: .pounds)
                == "bpm"
        )
        #expect(
            BrowseCardPresenter.displayUnit(field: "sleep_analysis", unit: "min", prefs: .pounds)
                == "min"
        )
    }

    // MARK: - sparklineState

    @Test("non-empty points -> .chart")
    func sparklineStateChart() {
        let points = [DataPoint(t: "2026-04-10", v: 1, n: 1)]
        let state = BrowseCardPresenter.sparklineState(points: points, isLoading: false)
        #expect(state == .chart(points: points))
    }

    @Test("chart takes precedence over loading when data is present")
    func sparklineStateChartBeatsLoading() {
        let points = [DataPoint(t: "2026-04-10", v: 1, n: 1)]
        let state = BrowseCardPresenter.sparklineState(points: points, isLoading: true)
        #expect(state == .chart(points: points))
    }

    @Test("nil points + loading -> .loading")
    func sparklineStateLoading() {
        let state = BrowseCardPresenter.sparklineState(points: nil, isLoading: true)
        #expect(state == .loading)
    }

    @Test("empty points + loading -> .loading")
    func sparklineStateEmptyArrayLoading() {
        let state = BrowseCardPresenter.sparklineState(points: [], isLoading: true)
        #expect(state == .loading)
    }

    @Test("nil points + not loading -> .empty placeholder")
    func sparklineStateEmpty() {
        let state = BrowseCardPresenter.sparklineState(points: nil, isLoading: false)
        #expect(state == .empty)
    }

    @Test("empty array + not loading -> .empty placeholder")
    func sparklineStateEmptyArray() {
        let state = BrowseCardPresenter.sparklineState(points: [], isLoading: false)
        #expect(state == .empty)
    }

    // MARK: - latestValueText

    @Test("nil points -> nil (caller renders placeholder)")
    func latestValueNilForNoPoints() {
        #expect(BrowseCardPresenter.latestValueText(field: "heart_rate", points: nil) == nil)
        #expect(BrowseCardPresenter.latestValueText(field: "heart_rate", points: []) == nil)
    }

    @Test("body_mass value goes through WeightFormatter (lb pref)")
    func latestValueBodyMassPounds() {
        let points = [DataPoint(t: "2026-04-10", v: 70, n: 1)]
        let text = BrowseCardPresenter.latestValueText(
            field: "body_mass",
            points: points,
            prefs: .pounds
        )
        // 70 kg -> 154.3 lb (one decimal, no unit — the unit is shown above).
        #expect(text == "154.3")
    }

    @Test("body_mass value goes through WeightFormatter (kg pref)")
    func latestValueBodyMassKilograms() {
        let points = [DataPoint(t: "2026-04-10", v: 70, n: 1)]
        let text = BrowseCardPresenter.latestValueText(
            field: "body_mass",
            points: points,
            prefs: .kilograms
        )
        #expect(text == "70.0")
    }

    @Test("small non-body_mass values get one decimal")
    func latestValueSmallNumber() {
        let points = [DataPoint(t: "2026-04-10", v: 6.4, n: 1)]
        let text = BrowseCardPresenter.latestValueText(field: "energy", points: points)
        #expect(text == "6.4")
    }

    @Test("values >= 10 drop decimals for a compact display")
    func latestValueLargeNumber() {
        let points = [DataPoint(t: "2026-04-10", v: 62, n: 1)]
        let text = BrowseCardPresenter.latestValueText(field: "heart_rate", points: points)
        #expect(text == "62")
    }

    @Test("uses the LAST point (chronological-last in the array)")
    func latestValueUsesLastPoint() {
        let points = [
            DataPoint(t: "2026-04-10", v: 60, n: 1),
            DataPoint(t: "2026-04-11", v: 65, n: 1),
            DataPoint(t: "2026-04-12", v: 58, n: 1),
        ]
        let text = BrowseCardPresenter.latestValueText(field: "heart_rate", points: points)
        #expect(text == "58")
    }
}
