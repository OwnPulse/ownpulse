// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("ExploreViewModel", .serialized)
@MainActor
struct ExploreViewModelTests {
    private func makePoints(_ values: [Double]) -> [DataPoint] {
        values.enumerated().map { i, v in
            DataPoint(t: "2026-04-\(String(format: "%02d", i + 10))", v: v, n: 1)
        }
    }

    @Test("loadSparklines populates sparklineData keyed by source.field")
    func loadSparklinesPopulatesMap() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, path, _ -> Any in
            #expect(path == Endpoints.batchSeries)
            return BatchSeriesResponse(series: [
                SeriesData(
                    source: "health_records",
                    field: "heart_rate",
                    unit: "bpm",
                    points: self.makePoints([60, 62, 58])
                ),
                SeriesData(
                    source: "health_records",
                    field: "body_mass",
                    unit: "kg",
                    points: self.makePoints([72.1, 72.0, 71.9])
                ),
            ])
        }

        let vm = ExploreViewModel(networkClient: mock)
        await vm.loadSparklines(source: "health_records", fields: ["heart_rate", "body_mass"])

        #expect(vm.sparklineData.count == 2)
        let hrKey = ExploreViewModel.sparklineKey(source: "health_records", field: "heart_rate")
        let bmKey = ExploreViewModel.sparklineKey(source: "health_records", field: "body_mass")
        #expect(vm.sparklineData[hrKey]?.count == 3)
        #expect(vm.sparklineData[bmKey]?.count == 3)
        // Loading markers clear after the request resolves.
        #expect(vm.sparklineLoadingSections.isEmpty)
    }

    @Test("loadSparklines does not touch unrelated fields in sparklineData")
    func loadSparklinesLeavesOthersUntouched() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ -> Any in
            BatchSeriesResponse(series: [
                SeriesData(
                    source: "health_records",
                    field: "heart_rate",
                    unit: "bpm",
                    points: self.makePoints([60, 62])
                ),
            ])
        }

        let vm = ExploreViewModel(networkClient: mock)
        let untouchedKey = ExploreViewModel.sparklineKey(source: "checkins", field: "energy")
        vm.sparklineData[untouchedKey] = self.makePoints([5, 6, 7])

        await vm.loadSparklines(source: "health_records", fields: ["heart_rate"])

        // Pre-existing entry is preserved.
        #expect(vm.sparklineData[untouchedKey]?.count == 3)
        // New entry is present.
        let hrKey = ExploreViewModel.sparklineKey(source: "health_records", field: "heart_rate")
        #expect(vm.sparklineData[hrKey]?.count == 2)
    }

    @Test("loadSparklines chunks requests when more than 10 fields are requested")
    func loadSparklinesPaginates() async {
        let mock = MockNetworkClient()
        var callCount = 0
        mock.requestHandler = { _, _, body -> Any in
            callCount += 1
            guard let req = body as? BatchSeriesRequest else {
                Issue.record("body was not a BatchSeriesRequest")
                return BatchSeriesResponse(series: [])
            }
            #expect(req.metrics.count <= 10)
            let series = req.metrics.map { spec in
                SeriesData(
                    source: spec.source,
                    field: spec.field,
                    unit: "bpm",
                    points: self.makePoints([1])
                )
            }
            return BatchSeriesResponse(series: series)
        }

        let fields = (0..<15).map { "metric_\($0)" }
        let vm = ExploreViewModel(networkClient: mock)
        await vm.loadSparklines(source: "health_records", fields: fields)

        #expect(callCount == 2)
        #expect(vm.sparklineData.count == 15)
    }

    @Test("loadSparklines skips fields already present in sparklineData")
    func loadSparklinesSkipsExisting() async {
        let mock = MockNetworkClient()
        var requestCount = 0
        mock.requestHandler = { _, _, body -> Any in
            requestCount += 1
            guard let req = body as? BatchSeriesRequest else {
                return BatchSeriesResponse(series: [])
            }
            return BatchSeriesResponse(series: req.metrics.map { spec in
                SeriesData(source: spec.source, field: spec.field, unit: "", points: [])
            })
        }

        let vm = ExploreViewModel(networkClient: mock)
        let existingKey = ExploreViewModel.sparklineKey(source: "health_records", field: "heart_rate")
        vm.sparklineData[existingKey] = [DataPoint(t: "2026-04-10", v: 60, n: 1)]

        await vm.loadSparklines(source: "health_records", fields: ["heart_rate"])

        #expect(requestCount == 0)
        // Pre-existing value is preserved.
        #expect(vm.sparklineData[existingKey]?.count == 1)
    }

    @Test("loadSparklines network failure leaves sparklineData empty and clears loading flag")
    func loadSparklinesFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ -> Any in
            throw NetworkError.serverError(statusCode: 500, body: "boom")
        }

        let vm = ExploreViewModel(networkClient: mock)
        await vm.loadSparklines(source: "health_records", fields: ["heart_rate"])

        let key = ExploreViewModel.sparklineKey(source: "health_records", field: "heart_rate")
        #expect(vm.sparklineData[key] == nil)
        #expect(vm.sparklineLoadingSections.isEmpty)
    }

    @Test("loadSparklines with empty field list is a no-op")
    func loadSparklinesEmptyFieldList() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ -> Any in
            Issue.record("unexpected network request for empty field list")
            return BatchSeriesResponse(series: [])
        }

        let vm = ExploreViewModel(networkClient: mock)
        await vm.loadSparklines(source: "health_records", fields: [])

        #expect(vm.sparklineData.isEmpty)
    }
}
