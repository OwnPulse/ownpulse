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

    /// 7-day sparkline data for browse cards, keyed by "<source>.<field>".
    /// Loaded lazily per section to avoid firing one request per card.
    var sparklineData: [String: [DataPoint]] = [:]
    /// Section keys currently being fetched (prevents duplicate requests).
    var sparklineLoadingSections: Set<String> = []

    private let networkClient: NetworkClientProtocol

    init(networkClient: NetworkClientProtocol) {
        self.networkClient = networkClient
    }

    /// Key format used in `sparklineData` — always `<source>.<field>` so the
    /// same field under different sources doesn't collide.
    static func sparklineKey(source: String, field: String) -> String {
        "\(source).\(field)"
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

    // MARK: - Sparklines (browse cards)

    /// Fetches 7-day daily sparkline data for a batch of metrics from a single
    /// source. Pages in groups of 10 (the backend's `batch-series` limit).
    /// Skips fields already loaded or currently loading.
    func loadSparklines(source: String, fields: [String]) async {
        // Filter out fields we've already fetched or are mid-fetch.
        let pending = fields.filter { field in
            let key = Self.sparklineKey(source: source, field: field)
            return sparklineData[key] == nil && !sparklineLoadingSections.contains(key)
        }
        guard !pending.isEmpty else { return }

        // Mark as loading up-front so re-triggers (scroll jitter) are no-ops.
        for field in pending {
            sparklineLoadingSections.insert(Self.sparklineKey(source: source, field: field))
        }

        let formatter = ISO8601DateFormatter()
        let now = Date()
        let sevenDaysAgo = Calendar.current.date(byAdding: .day, value: -7, to: now) ?? now

        // Chunk by 10 per the backend cap.
        for chunk in pending.chunked(into: 10) {
            let request = BatchSeriesRequest(
                metrics: chunk.map { MetricSpec(source: source, field: $0) },
                start: formatter.string(from: sevenDaysAgo),
                end: formatter.string(from: now),
                resolution: "1d"
            )
            do {
                let response: BatchSeriesResponse = try await networkClient.request(
                    method: "POST",
                    path: Endpoints.batchSeries,
                    body: request
                )
                for series in response.series {
                    let key = Self.sparklineKey(source: series.source, field: series.field)
                    sparklineData[key] = series.points
                }
            } catch {
                logger.error("Failed to load sparklines for \(source, privacy: .public): \(error.localizedDescription, privacy: .public)")
            }
            // Always clear the loading markers for this chunk so retry is possible.
            for field in chunk {
                sparklineLoadingSections.remove(Self.sparklineKey(source: source, field: field))
            }
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

private extension Array {
    /// Splits the array into chunks of at most `size` contiguous elements.
    func chunked(into size: Int) -> [[Element]] {
        guard size > 0 else { return [self] }
        return stride(from: 0, to: count, by: size).map {
            Array(self[$0..<Swift.min($0 + size, count)])
        }
    }
}
