// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import Testing
@testable import OwnPulse

/// Covers the C7 chart-data transforms that back the Phase 3b Swift Charts
/// dashboard cards. These are pure functions, so they're tested without a
/// simulator. The transforms feed the `WeeklySummaryCard` bar chart and the
/// `HeroMetricCard` color lookup; both must stay in lockstep with the shared
/// `ChartColors` token source.
@Suite("DashboardChartData")
struct DashboardChartDataTests {
    private func summary(
        checkins: Int = 5,
        records: Int = 42,
        interventions: Int = 3,
        observations: Int = 2
    ) -> DashboardSummary {
        DashboardSummary(
            latestCheckin: nil,
            checkinCount7d: checkins,
            healthRecordCount7d: records,
            interventionCount7d: interventions,
            observationCount7d: observations,
            latestLabDate: nil,
            pendingFriendShares: 0
        )
    }

    // MARK: - weeklyBars

    @Test("weeklyBars maps every summary count into a labeled bar, in order")
    func weeklyBarsOrderAndValues() {
        let bars = DashboardChartData.weeklyBars(from: summary())
        #expect(bars.count == 4)
        #expect(bars.map(\.label) == ["Check-ins", "Records", "Interventions", "Observations"])
        #expect(bars.map(\.value) == [5, 42, 3, 2])
    }

    @Test("weeklyBars carries a distinct color index per category for the fallback cycle")
    func weeklyBarsColorIndices() {
        let bars = DashboardChartData.weeklyBars(from: summary())
        #expect(bars.map(\.colorIndex) == [0, 1, 2, 3])
        // Each bar's color resolves through the shared token source; the
        // health-records bar is keyed (heart_rate) so it must NOT be a fallback.
        let recordsBar = bars[1]
        #expect(
            ChartColors.color(for: recordsBar.colorMetric, index: recordsBar.colorIndex)
                == ChartColors.metric["heart_rate"]
        )
        // The unkeyed categories fall back deterministically by index.
        let checkinsBar = bars[0]
        #expect(
            ChartColors.color(for: checkinsBar.colorMetric, index: checkinsBar.colorIndex)
                == ChartColors.fallback[0]
        )
    }

    @Test("weeklyBars handles all-zero counts without dropping categories")
    func weeklyBarsZero() {
        let bars = DashboardChartData.weeklyBars(
            from: summary(checkins: 0, records: 0, interventions: 0, observations: 0)
        )
        #expect(bars.count == 4)
        #expect(bars.allSatisfy { $0.value == 0 })
    }

    @Test("WeeklyBar id is its label so SwiftUI ForEach stays stable")
    func weeklyBarIdentity() {
        let bar = DashboardChartData.WeeklyBar(
            label: "Records", value: 42, colorMetric: "heart_rate", colorIndex: 1
        )
        #expect(bar.id == "Records")
    }

    // MARK: - weeklyAccessibilitySummary

    @Test("weeklyAccessibilitySummary spells out every count for VoiceOver")
    func accessibilitySummary() {
        let text = DashboardChartData.weeklyAccessibilitySummary(from: summary())
        #expect(text == "Check-ins 5, Records 42, Interventions 3, Observations 2")
    }

    @Test("weeklyAccessibilitySummary includes zero counts (never color-only)")
    func accessibilitySummaryZero() {
        let text = DashboardChartData.weeklyAccessibilitySummary(
            from: summary(checkins: 0, records: 0, interventions: 0, observations: 0)
        )
        #expect(text == "Check-ins 0, Records 0, Interventions 0, Observations 0")
    }

    // MARK: - colorKey

    @Test("colorKey passes a non-empty field through, and resting_heart_rate resolves to the heart_rate token color")
    func colorKeyPassthrough() {
        // The hero metric's real backend field is resting_heart_rate; it must
        // pass through unchanged and resolve (via the ChartColors alias layer)
        // to the dedicated heart_rate token color — NOT the fallback.
        #expect(DashboardChartData.colorKey(forField: "resting_heart_rate") == "resting_heart_rate")
        let heroColor = ChartColors.color(
            for: DashboardChartData.colorKey(forField: "resting_heart_rate"), index: 0
        )
        #expect(heroColor == ChartColors.metric["heart_rate"])
        #expect(heroColor != ChartColors.fallback[0])
    }

    @Test("colorKey defaults an empty field to the canonical hero field, which is colored")
    func colorKeyEmptyDefault() {
        #expect(DashboardChartData.colorKey(forField: "") == DashboardChartData.defaultHeroField)
        // The canonical default resolves to the heart_rate token color, so an
        // unpopulated hero card never renders a fallback color.
        #expect(
            ChartColors.color(for: DashboardChartData.colorKey(forField: ""), index: 0)
                == ChartColors.metric["heart_rate"]
        )
    }

    @Test("the canonical hero field aliases to the heart_rate token color")
    func defaultHeroFieldIsKeyed() {
        #expect(DashboardChartData.defaultHeroField == "resting_heart_rate")
        #expect(
            ChartColors.color(for: DashboardChartData.defaultHeroField, index: 0)
                == ChartColors.metric["heart_rate"]
        )
    }
}
