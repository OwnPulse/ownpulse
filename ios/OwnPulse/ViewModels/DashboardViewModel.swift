// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Observation
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "dashboard")

@Observable
@MainActor
final class DashboardViewModel {
    // MARK: - State

    enum LoadState: Sendable {
        case idle
        case loading
        case loaded
        case error(String)
    }

    var summaryState: LoadState = .idle
    var summary: DashboardSummary?
    var sparklines: [SeriesData] = []
    var insights: [Insight] = []
    var heroSeries: [DataPoint] = []
    var heroMetricName: String = ""
    var heroMetricUnit: String = ""
    var heroCurrentValue: String = ""
    var heroTrendText: String = ""
    var heroTrendIsPositive: Bool = true
    var lastSyncDate: Date?

    private let networkClient: NetworkClientProtocol
    private let syncEngine: SyncEngine?
    private let widgetPublisher: WidgetDataPublisher

    init(
        networkClient: NetworkClientProtocol,
        syncEngine: SyncEngine? = nil,
        widgetPublisher: WidgetDataPublisher = WidgetDataPublisher()
    ) {
        self.networkClient = networkClient
        self.syncEngine = syncEngine
        self.widgetPublisher = widgetPublisher
    }

    // MARK: - Fetch All Dashboard Data

    func loadDashboard() async {
        summaryState = .loading

        do {
            let dashboardSummary: DashboardSummary = try await networkClient.request(
                method: "GET",
                path: Endpoints.dashboardSummary,
                body: nil as String?
            )
            summary = dashboardSummary

            summaryState = .loaded
        } catch {
            logger.error("Failed to load dashboard: \(error.localizedDescription, privacy: .public)")
            Task { await FlowTracker.shared.track(event: "flow", flow: "dashboard_load", outcome: "error") }
            #if DEBUG
            summaryState = .error(error.localizedDescription)
            #else
            summaryState = .error("Unable to load dashboard. Pull to retry.")
            #endif
        }

        // Load sparklines and insights in parallel, non-blocking
        async let sparklineTask: Void = loadSparklines()
        async let insightTask: Void = loadInsights()
        async let heroTask: Void = loadHeroMetric()
        _ = await (sparklineTask, insightTask, heroTask)

        // Snapshot the freshly-loaded values into the shared app group so the
        // lock-screen widgets reflect today's data. This is the on-device
        // completion path where both the check-in status and the latest hero
        // metric are known. Widgets are read-only consumers of this snapshot.
        publishWidgetSnapshot()
    }

    /// Build a ``WidgetSnapshot`` from the current view-model state and hand
    /// it to the publisher. Exposed (internal) for unit testing.
    func publishWidgetSnapshot() {
        let snapshot = WidgetSnapshot(
            checkinFilledToday: summary?.latestCheckin?.isToday ?? false,
            heroMetricName: heroMetricName.isEmpty ? "Resting Heart Rate" : heroMetricName,
            heroMetricValue: heroCurrentValue.isEmpty ? "—" : heroCurrentValue,
            heroMetricUnit: heroMetricUnit.isEmpty ? "bpm" : heroMetricUnit,
            heroTrendText: heroTrendText,
            heroTrendIsPositive: heroTrendIsPositive,
            lastUpdated: Date()
        )
        widgetPublisher.publish(snapshot)
    }

    // MARK: - Sparklines

    func loadSparklines() async {
        let formatter = ISO8601DateFormatter()
        let now = Date()
        let sevenDaysAgo = Calendar.current.date(byAdding: .day, value: -7, to: now) ?? now

        // Resolution values must match the backend's `Resolution` enum —
        // `daily` / `weekly` / `monthly`, see backend/api/src/models/explore.rs.
        // Sending anything else (e.g. `"1d"`) returns 422.
        let request = BatchSeriesRequest(
            metrics: [
                MetricSpec(source: "checkins", field: "energy"),
                MetricSpec(source: "checkins", field: "mood"),
                MetricSpec(source: "checkins", field: "focus"),
                MetricSpec(source: "checkins", field: "recovery"),
                MetricSpec(source: "checkins", field: "libido"),
            ],
            start: formatter.string(from: sevenDaysAgo),
            end: formatter.string(from: now),
            resolution: "daily"
        )

        do {
            let response: BatchSeriesResponse = try await networkClient.request(
                method: "POST",
                path: Endpoints.batchSeries,
                body: request
            )
            sparklines = response.series
        } catch {
            logger.error("Failed to load sparklines: \(error.localizedDescription, privacy: .public)")
        }
    }

    // MARK: - Insights

    func loadInsights() async {
        do {
            let fetchedInsights: [Insight] = try await networkClient.request(
                method: "GET",
                path: Endpoints.insights,
                body: nil as String?
            )
            insights = fetchedInsights
        } catch {
            logger.error("Failed to load insights: \(error.localizedDescription, privacy: .public)")
        }
    }

    func dismissInsight(_ insight: Insight) {
        insights.removeAll { $0.id == insight.id }
    }

    // MARK: - Hero Metric

    func loadHeroMetric() async {
        let formatter = ISO8601DateFormatter()
        let now = Date()
        let thirtyDaysAgo = Calendar.current.date(byAdding: .day, value: -30, to: now) ?? now

        let request = BatchSeriesRequest(
            metrics: [MetricSpec(source: "health_records", field: "resting_heart_rate")],
            start: formatter.string(from: thirtyDaysAgo),
            end: formatter.string(from: now),
            resolution: "daily"
        )

        do {
            let response: BatchSeriesResponse = try await networkClient.request(
                method: "POST",
                path: Endpoints.batchSeries,
                body: request
            )
            if let series = response.series.first, !series.points.isEmpty {
                heroSeries = series.points
                heroMetricName = "Resting Heart Rate"
                heroMetricUnit = series.unit.isEmpty ? "bpm" : series.unit
                if let last = series.points.last {
                    heroCurrentValue = String(format: "%.0f", last.v)
                }
                computeTrend(points: series.points)
            }
        } catch {
            logger.error("Failed to load hero metric: \(error.localizedDescription, privacy: .public)")
        }
    }

    private func computeTrend(points: [DataPoint]) {
        guard points.count >= 2 else {
            heroTrendText = ""
            return
        }
        let values = points.map(\.v)
        let avg = values.reduce(0, +) / Double(values.count)
        guard avg > 0, let latest = values.last else {
            heroTrendText = ""
            return
        }
        let pctChange = ((latest - avg) / avg) * 100
        // Round first, then derive sign/label from the rounded value so we
        // never render a contradictory "-0%" (a tiny negative that rounds to
        // zero would otherwise print with a leading minus).
        let rounded = (pctChange).rounded()
        let direction = rounded > 0 ? "+" : ""
        heroTrendText = "\(direction)\(String(format: "%.0f", rounded))% vs 30d avg"
        // POLARITY IS RESTING-HR-ONLY. The hero metric is currently hardcoded
        // to resting heart rate (see loadHeroMetric), where *lower* is the
        // healthy direction — so a non-positive change is "good" (sage tint).
        // This is WRONG for higher-is-better metrics (HRV, sleep duration). If
        // the hero metric is ever generalized, tie this polarity to the metric
        // type rather than assuming lower-is-better, or the lock-screen tint
        // will mislead.
        heroTrendIsPositive = rounded <= 0
    }

    // MARK: - Sync

    func performSync() async {
        await syncEngine?.sync()
        lastSyncDate = await syncEngine?.lastSyncDate
    }
}
