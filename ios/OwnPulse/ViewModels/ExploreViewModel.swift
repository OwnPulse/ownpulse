// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Observation
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "explore")

enum DateRangePreset: String, CaseIterable, Sendable {
    case oneWeek = "1W"
    case oneMonth = "1M"
    case threeMonths = "3M"
    case sixMonths = "6M"
    case oneYear = "1Y"

    var daysBack: Int {
        switch self {
        case .oneWeek: return 7
        case .oneMonth: return 30
        case .threeMonths: return 90
        case .sixMonths: return 180
        case .oneYear: return 365
        }
    }

    var resolution: String {
        switch self {
        case .oneWeek, .oneMonth: return "daily"
        case .threeMonths, .sixMonths: return "weekly"
        case .oneYear: return "monthly"
        }
    }
}

@Observable
@MainActor
final class ExploreViewModel {
    // MARK: - State

    enum LoadState: Sendable {
        case idle
        case loading
        case loaded
        case error(String)
    }

    var metrics: [MetricSourceGroup] = []
    var selectedMetrics: [MetricSpec] = []
    var seriesData: [SeriesData] = []
    var interventions: [InterventionMarker] = []
    var dateRange: DateRangePreset = .oneMonth
    var showMovingAverage: Bool = false
    var loadState: LoadState = .idle
    var metricsLoadState: LoadState = .idle

    private let networkClient: NetworkClientProtocol

    init(networkClient: NetworkClientProtocol) {
        self.networkClient = networkClient
    }

    // MARK: - Load available metrics

    func loadMetrics() async {
        metricsLoadState = .loading
        do {
            let response: MetricsResponse = try await networkClient.request(
                method: "GET",
                path: Endpoints.exploreMetrics,
                body: nil as String?
            )
            metrics = response.sources
            metricsLoadState = .loaded
        } catch {
            logger.error("Failed to load explore metrics: \(error.localizedDescription, privacy: .public)")
            metricsLoadState = .error("Failed to load metrics")
        }
    }

    // MARK: - Load time-series data

    func loadSeries() async {
        guard !selectedMetrics.isEmpty else {
            seriesData = []
            return
        }

        loadState = .loading

        let formatter = ISO8601DateFormatter()
        let now = Date()
        let startDate = Calendar.current.date(byAdding: .day, value: -dateRange.daysBack, to: now) ?? now

        let request = BatchSeriesRequest(
            metrics: selectedMetrics,
            start: formatter.string(from: startDate),
            end: formatter.string(from: now),
            resolution: dateRange.resolution
        )

        do {
            let response: BatchSeriesResponse = try await networkClient.request(
                method: "POST",
                path: Endpoints.exploreSeries,
                body: request
            )
            seriesData = response.series
            loadState = .loaded
        } catch {
            logger.error("Failed to load explore series: \(error.localizedDescription, privacy: .public)")
            loadState = .error("Failed to load chart data")
        }
    }

    // MARK: - Load interventions

    func loadInterventions() async {
        let formatter = ISO8601DateFormatter()
        let now = Date()
        let startDate = Calendar.current.date(byAdding: .day, value: -dateRange.daysBack, to: now) ?? now

        let path = "\(Endpoints.interventions)?start=\(formatter.string(from: startDate))&end=\(formatter.string(from: now))"

        do {
            let markers: [InterventionMarker] = try await networkClient.request(
                method: "GET",
                path: path,
                body: nil as String?
            )
            interventions = markers
        } catch {
            logger.error("Failed to load interventions: \(error.localizedDescription, privacy: .public)")
        }
    }

    // MARK: - Metric selection

    func selectMetric(_ spec: MetricSpec) {
        guard !selectedMetrics.contains(where: { $0.source == spec.source && $0.field == spec.field }) else { return }
        guard selectedMetrics.count < 5 else { return }
        selectedMetrics.append(spec)
        Task { await loadSeries() }
    }

    func removeMetric(_ spec: MetricSpec) {
        selectedMetrics.removeAll { $0.source == spec.source && $0.field == spec.field }
        seriesData.removeAll { $0.source == spec.source && $0.field == spec.field }
        if selectedMetrics.isEmpty {
            loadState = .idle
        }
    }

    // MARK: - Date range

    func setDateRange(_ range: DateRangePreset) {
        dateRange = range
        Task {
            async let seriesTask: Void = loadSeries()
            async let interventionsTask: Void = loadInterventions()
            _ = await (seriesTask, interventionsTask)
        }
    }

    // MARK: - Health Overview preset

    func loadHealthOverviewPreset() {
        selectedMetrics = [
            MetricSpec(source: "health_records", field: "body_mass"),
            MetricSpec(source: "health_records", field: "heart_rate"),
            MetricSpec(source: "health_records", field: "sleep_analysis"),
        ]
        showMovingAverage = true
        Task {
            async let seriesTask: Void = loadSeries()
            async let interventionsTask: Void = loadInterventions()
            _ = await (seriesTask, interventionsTask)
        }
    }
}
