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
}
