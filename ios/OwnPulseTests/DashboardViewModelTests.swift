// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("DashboardViewModel", .serialized)
@MainActor
struct DashboardViewModelTests {
    // MARK: - Helpers

    private func makeSummary(
        checkinCount: Int = 5,
        healthRecords: Int = 42,
        interventions: Int = 3,
        observations: Int = 2
    ) -> DashboardSummary {
        DashboardSummary(
            latestCheckin: LatestCheckin(
                energy: 7, mood: 8, focus: 6, recovery: 7, libido: 5,
                date: ISO8601DateFormatter().string(from: Date())
            ),
            checkinCount7d: checkinCount,
            healthRecordCount7d: healthRecords,
            interventionCount7d: interventions,
            observationCount7d: observations,
            latestLabDate: nil,
            pendingFriendShares: 0
        )
    }

    private func makeBatchResponse() -> BatchSeriesResponse {
        BatchSeriesResponse(series: [
            SeriesData(source: "checkins", field: "energy", unit: "", points: [
                DataPoint(t: "2026-03-21", v: 6, n: 1),
                DataPoint(t: "2026-03-22", v: 7, n: 1),
                DataPoint(t: "2026-03-23", v: 8, n: 1),
            ]),
            SeriesData(source: "checkins", field: "mood", unit: "", points: [
                DataPoint(t: "2026-03-21", v: 5, n: 1),
                DataPoint(t: "2026-03-22", v: 6, n: 1),
            ]),
        ])
    }

    private func makeInsights() -> [Insight] {
        [
            Insight(
                id: "ins-1",
                insightType: "correlation",
                headline: "Sleep correlates with mood",
                detail: "Your mood scores are higher after 7+ hours of sleep.",
                createdAt: "2026-03-28T10:00:00Z"
            ),
            Insight(
                id: "ins-2",
                insightType: "trend",
                headline: "Energy trending up",
                detail: nil,
                createdAt: "2026-03-28T10:00:00Z"
            ),
        ]
    }

    // MARK: - Dashboard Summary

    @Test("loadDashboard success populates summary and transitions to loaded")
    func loadDashboardSuccess() async {
        let mock = MockNetworkClient()
        let summary = makeSummary()
        let batchResponse = makeBatchResponse()
        let insights = makeInsights()

        mock.requestHandler = { method, path, _ in
            if path == Endpoints.dashboardSummary {
                return summary
            } else if path == Endpoints.batchSeries {
                return batchResponse
            } else if path == Endpoints.insights {
                return insights
            }
            return summary
        }

        let vm = DashboardViewModel(networkClient: mock)
        #expect(vm.summaryState == .idle)

        await vm.loadDashboard()

        #expect(vm.summaryState == .loaded)
        #expect(vm.summary != nil)
        #expect(vm.summary?.checkinCount7d == 5)
        #expect(vm.summary?.healthRecordCount7d == 42)
    }

    @Test("loadDashboard failure transitions to error state")
    func loadDashboardFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, path, _ -> Any in
            if path == Endpoints.dashboardSummary {
                throw NetworkError.serverError(statusCode: 500, body: "internal")
            } else if path == Endpoints.insights {
                return [Insight]() as [Insight]
            } else {
                return BatchSeriesResponse(series: [])
            }
        }

        let vm = DashboardViewModel(networkClient: mock)
        await vm.loadDashboard()

        if case .error(let msg) = vm.summaryState {
            #expect(!msg.isEmpty)
        } else {
            Issue.record("Expected error state, got \(vm.summaryState)")
        }
    }

    @Test("loadDashboard network error transitions to error state")
    func loadDashboardNetworkError() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ -> Any in
            throw NetworkError.unauthorized
        }

        let vm = DashboardViewModel(networkClient: mock)
        await vm.loadDashboard()

        if case .error = vm.summaryState {
            // Expected
        } else {
            Issue.record("Expected error state")
        }
    }

    // MARK: - Sparklines

    @Test("loadSparklines populates sparkline data")
    func loadSparklines() async {
        let mock = MockNetworkClient()
        let batchResponse = makeBatchResponse()

        mock.requestHandler = { _, _, _ in batchResponse }

        let vm = DashboardViewModel(networkClient: mock)
        await vm.loadSparklines()

        #expect(vm.sparklines.count == 2)
        #expect(vm.sparklines[0].field == "energy")
        #expect(vm.sparklines[0].points.count == 3)
    }

    @Test("loadSparklines failure does not crash, leaves empty")
    func loadSparklinesFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "error")
        }

        let vm = DashboardViewModel(networkClient: mock)
        await vm.loadSparklines()

        #expect(vm.sparklines.isEmpty)
    }

    // MARK: - Insights

    @Test("loadInsights populates insights list")
    func loadInsights() async {
        let mock = MockNetworkClient()
        let insights = makeInsights()
        mock.requestHandler = { _, _, _ in insights }

        let vm = DashboardViewModel(networkClient: mock)
        await vm.loadInsights()

        #expect(vm.insights.count == 2)
        #expect(vm.insights[0].headline == "Sleep correlates with mood")
    }

    @Test("loadInsights failure leaves empty list")
    func loadInsightsFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "error")
        }

        let vm = DashboardViewModel(networkClient: mock)
        await vm.loadInsights()

        #expect(vm.insights.isEmpty)
    }

    @Test("dismissInsight removes the insight from the list")
    func dismissInsight() async {
        let mock = MockNetworkClient()
        let insights = makeInsights()
        mock.requestHandler = { _, _, _ in insights }

        let vm = DashboardViewModel(networkClient: mock)
        await vm.loadInsights()

        #expect(vm.insights.count == 2)
        vm.dismissInsight(insights[0])
        #expect(vm.insights.count == 1)
        #expect(vm.insights[0].id == "ins-2")
    }

    // MARK: - Hero Metric

    @Test("loadHeroMetric sets hero series and computes trend")
    func loadHeroMetric() async {
        let mock = MockNetworkClient()
        let heroResponse = BatchSeriesResponse(series: [
            SeriesData(source: "health_records", field: "resting_heart_rate", unit: "bpm", points: [
                DataPoint(t: "2026-03-01", v: 60, n: 1),
                DataPoint(t: "2026-03-15", v: 58, n: 1),
                DataPoint(t: "2026-03-28", v: 56, n: 1),
            ]),
        ])
        mock.requestHandler = { _, _, _ in heroResponse }

        let vm = DashboardViewModel(networkClient: mock)
        await vm.loadHeroMetric()

        #expect(vm.heroSeries.count == 3)
        #expect(vm.heroMetricName == "Resting Heart Rate")
        #expect(vm.heroMetricUnit == "bpm")
        #expect(vm.heroCurrentValue == "56")
        #expect(!vm.heroTrendText.isEmpty)
    }

    @Test("trend never renders a contradictory -0%")
    func trendNeverNegativeZero() async {
        // A latest value fractionally below the average yields a tiny negative
        // pctChange that rounds to 0 — it must print "+0%", not "-0%".
        let mock = MockNetworkClient()
        let heroResponse = BatchSeriesResponse(series: [
            SeriesData(source: "health_records", field: "resting_heart_rate", unit: "bpm", points: [
                DataPoint(t: "2026-03-01", v: 60.0, n: 1),
                DataPoint(t: "2026-03-28", v: 59.99, n: 1),
            ]),
        ])
        mock.requestHandler = { _, _, _ in heroResponse }

        let vm = DashboardViewModel(networkClient: mock)
        await vm.loadHeroMetric()

        #expect(!vm.heroTrendText.contains("-0%"))
        #expect(vm.heroTrendText.hasPrefix("+0%"))
    }

    @Test("trend polarity treats lower resting HR as the good direction")
    func trendPolarityLowerIsGood() async {
        let mock = MockNetworkClient()
        // Latest is below the average -> a decrease in resting HR -> good.
        let heroResponse = BatchSeriesResponse(series: [
            SeriesData(source: "health_records", field: "resting_heart_rate", unit: "bpm", points: [
                DataPoint(t: "2026-03-01", v: 70, n: 1),
                DataPoint(t: "2026-03-28", v: 55, n: 1),
            ]),
        ])
        mock.requestHandler = { _, _, _ in heroResponse }

        let vm = DashboardViewModel(networkClient: mock)
        await vm.loadHeroMetric()

        #expect(vm.heroTrendIsPositive == true) // lower HR is "positive" for the user
        // ...but the value went DOWN, so the arrow must point down even though
        // the change is "good". The grayscale arrow follows the data, not the
        // polarity. (This is the regression code-review flagged: the arrow used
        // to be derived from the polarity flag and pointed up here.)
        #expect(vm.heroTrendDirection == .down)
        #expect(vm.heroTrendText.hasPrefix("-")) // arrow direction matches the sign of the number
    }

    @Test("hero arrow direction matches the sign of the change, not the polarity")
    func trendArrowFollowsDataNotPolarity() async {
        // Resting HR RISING (bad for the user) must render an UP arrow.
        let mock = MockNetworkClient()
        let heroResponse = BatchSeriesResponse(series: [
            SeriesData(source: "health_records", field: "resting_heart_rate", unit: "bpm", points: [
                DataPoint(t: "2026-03-01", v: 55, n: 1),
                DataPoint(t: "2026-03-28", v: 70, n: 1),
            ]),
        ])
        mock.requestHandler = { _, _, _ in heroResponse }

        let vm = DashboardViewModel(networkClient: mock)
        await vm.loadHeroMetric()

        #expect(vm.heroTrendDirection == .up)
        #expect(vm.heroTrendText.hasPrefix("+"))
        #expect(vm.heroTrendIsPositive == false) // rising HR is "bad" — color polarity differs from arrow
    }

    @Test("loadHeroMetric failure leaves hero empty")
    func loadHeroMetricFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "error")
        }

        let vm = DashboardViewModel(networkClient: mock)
        await vm.loadHeroMetric()

        #expect(vm.heroSeries.isEmpty)
        #expect(vm.heroCurrentValue == "")
    }

    @Test("loadHeroMetric populates heroMetricFieldKey from the loaded series field")
    func loadHeroMetricSetsFieldKey() async {
        // C7: the card colors its chart from this field. It must reflect the
        // real series field the backend returns, not a hardcoded value.
        let mock = MockNetworkClient()
        let heroResponse = BatchSeriesResponse(series: [
            SeriesData(source: "health_records", field: "resting_heart_rate", unit: "bpm", points: [
                DataPoint(t: "2026-03-01", v: 60, n: 1),
                DataPoint(t: "2026-03-28", v: 56, n: 1),
            ]),
        ])
        mock.requestHandler = { _, _, _ in heroResponse }

        let vm = DashboardViewModel(networkClient: mock)
        await vm.loadHeroMetric()

        #expect(vm.heroMetricFieldKey == "resting_heart_rate")
        // And that field resolves to the dedicated heart_rate token color.
        #expect(
            ChartColors.color(for: vm.heroMetricFieldKey, index: 0) == ChartColors.metric["heart_rate"]
        )
    }

    @Test("heroMetricFieldKey keeps the canonical default when no series loads")
    func heroMetricFieldKeyDefaultOnEmpty() async {
        let mock = MockNetworkClient()
        // Empty series list — loadHeroMetric must not overwrite the default.
        mock.requestHandler = { _, _, _ in BatchSeriesResponse(series: []) }

        let vm = DashboardViewModel(networkClient: mock)
        #expect(vm.heroMetricFieldKey == DashboardChartData.defaultHeroField)

        await vm.loadHeroMetric()

        #expect(vm.heroMetricFieldKey == DashboardChartData.defaultHeroField)
    }

    @Test("heroMetricFieldKey keeps the canonical default on error")
    func heroMetricFieldKeyDefaultOnError() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "error")
        }

        let vm = DashboardViewModel(networkClient: mock)
        await vm.loadHeroMetric()

        #expect(vm.heroMetricFieldKey == DashboardChartData.defaultHeroField)
    }

    // MARK: - Widget Snapshot Publishing

    /// In-memory store so the publisher can be observed without a real
    /// app-group container.
    private final class MemStore: WidgetDefaultsStore, @unchecked Sendable {
        private let lock = NSLock()
        private var storage: [String: Data] = [:]
        func data(forKey key: String) -> Data? {
            lock.lock(); defer { lock.unlock() }; return storage[key]
        }
        func set(_ data: Data?, forKey key: String) {
            lock.lock(); storage[key] = data; lock.unlock()
        }
    }

    @Test("loadDashboard publishes a widget snapshot reflecting today's data")
    func loadDashboardPublishesWidgetSnapshot() async {
        let mock = MockNetworkClient()
        let summary = makeSummary()
        let heroResponse = BatchSeriesResponse(series: [
            SeriesData(source: "health_records", field: "resting_heart_rate", unit: "bpm", points: [
                DataPoint(t: "2026-03-01", v: 60, n: 1),
                DataPoint(t: "2026-03-28", v: 56, n: 1),
            ]),
        ])
        mock.requestHandler = { _, path, _ in
            if path == Endpoints.dashboardSummary { return summary }
            if path == Endpoints.insights { return [Insight]() }
            return heroResponse
        }

        let store = MemStore()
        let publisher = WidgetDataPublisher(store: store, reload: {})
        let vm = DashboardViewModel(networkClient: mock, widgetPublisher: publisher)

        await vm.loadDashboard()

        let snapshot = publisher.load()
        #expect(snapshot != nil)
        // makeSummary() stamps the latest check-in with today's date.
        #expect(snapshot?.checkinFilledToday == true)
        #expect(snapshot?.heroMetricName == "Resting Heart Rate")
        #expect(snapshot?.heroMetricValue == "56")
        #expect(snapshot?.heroMetricUnit == "bpm")
        // 60 -> 56 is a decrease: the direction the widget snapshot carries
        // (which drives the widget's arrow + Wong color) must be .down. The
        // color-is-not-red assertion lives in TrendIndicatorTests.
        #expect(snapshot?.heroTrendDirection == .down)
    }

    @Test("publishWidgetSnapshot falls back to placeholders when no data loaded")
    func publishWidgetSnapshotFallsBack() {
        let mock = MockNetworkClient()
        let store = MemStore()
        let publisher = WidgetDataPublisher(store: store, reload: {})
        let vm = DashboardViewModel(networkClient: mock, widgetPublisher: publisher)

        vm.publishWidgetSnapshot()

        let snapshot = publisher.load()
        #expect(snapshot?.checkinFilledToday == false)
        #expect(snapshot?.heroMetricValue == "—")
        #expect(snapshot?.heroMetricName == "Resting Heart Rate")
    }
}
