// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("SourcePreferenceWizardViewModel", .serialized)
@MainActor
struct SourcePreferenceWizardTests {
    private func metric(_ type: String, _ sources: [(String, Int)]) -> OverlapMetric {
        OverlapMetric(
            metricType: type,
            sources: sources.map { OverlapSource(source: $0.0, recordCount: $0.1) }
        )
    }

    @Test("loadOverlaps populates metrics and pre-selects highest-count source")
    func loadPopulatesAndPreselects() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { method, path, _ -> Any in
            #expect(method == "GET")
            #expect(path == Endpoints.sourcesOverlapScan)
            return OverlapScanResponse(metrics: [
                self.metric("heart_rate", [("garmin", 120), ("oura", 90)]),
                self.metric("hrv", [("oura", 30), ("garmin", 10)]),
            ])
        }

        let vm = SourcePreferenceWizardViewModel(networkClient: mock)
        await vm.loadOverlaps()

        #expect(vm.state == .ready)
        #expect(vm.hasConflicts)
        #expect(vm.metrics.count == 2)
        // Pre-selection picks the first (highest-count) source per metric.
        #expect(vm.selections["heart_rate"] == "garmin")
        #expect(vm.selections["hrv"] == "oura")
    }

    @Test("loadOverlaps with no overlaps sets empty state")
    func loadEmpty() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ -> Any in
            OverlapScanResponse(metrics: [])
        }

        let vm = SourcePreferenceWizardViewModel(networkClient: mock)
        await vm.loadOverlaps()

        #expect(vm.state == .empty)
        #expect(!vm.hasConflicts)
    }

    @Test("loadOverlaps surfaces a failure state on error")
    func loadFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ -> Any in
            throw NetworkError.serverError(statusCode: 500, body: "boom")
        }

        let vm = SourcePreferenceWizardViewModel(networkClient: mock)
        await vm.loadOverlaps()

        if case .failed = vm.state {
            // expected
        } else {
            Issue.record("expected failed state, got \(vm.state)")
        }
    }

    @Test("saveSelections POSTs one preference per metric and finishes")
    func saveWritesEachMetric() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { method, path, body -> Any in
            if path == Endpoints.sourcesOverlapScan {
                return OverlapScanResponse(metrics: [
                    self.metric("heart_rate", [("garmin", 120), ("oura", 90)]),
                    self.metric("hrv", [("oura", 30), ("garmin", 10)]),
                ])
            }
            // Otherwise it's a source-preference upsert.
            #expect(method == "POST")
            #expect(path == Endpoints.sourcePreferences)
            let req = try #require(body as? UpsertSourcePreferenceRequest)
            return SourcePreferenceResponse(
                id: UUID().uuidString,
                metricType: req.metricType,
                preferredSource: req.preferredSource
            )
        }

        let vm = SourcePreferenceWizardViewModel(networkClient: mock)
        await vm.loadOverlaps()
        // Override one selection to confirm the user's choice is honored.
        vm.selections["heart_rate"] = "oura"
        await vm.saveSelections()

        #expect(vm.state == .finished)
        // One GET scan + two POST upserts.
        let posts = mock.requestCalls.filter { $0.path == Endpoints.sourcePreferences }
        #expect(posts.count == 2)
    }

    @Test("saveSelections reports failure when a write fails")
    func saveFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, path, _ -> Any in
            if path == Endpoints.sourcesOverlapScan {
                return OverlapScanResponse(metrics: [
                    self.metric("heart_rate", [("garmin", 120), ("oura", 90)]),
                ])
            }
            throw NetworkError.serverError(statusCode: 500, body: "boom")
        }

        let vm = SourcePreferenceWizardViewModel(networkClient: mock)
        await vm.loadOverlaps()
        await vm.saveSelections()

        if case .failed = vm.state {
            // expected
        } else {
            Issue.record("expected failed state, got \(vm.state)")
        }
    }

    @Test("saveSelections is a no-op when not in ready state")
    func saveNoOpWhenNotReady() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ -> Any in
            OverlapScanResponse(metrics: [])
        }

        let vm = SourcePreferenceWizardViewModel(networkClient: mock)
        await vm.loadOverlaps() // -> .empty
        await vm.saveSelections()

        // No POSTs issued; state unchanged.
        #expect(vm.state == .empty)
        #expect(mock.requestCalls.filter { $0.path == Endpoints.sourcePreferences }.isEmpty)
    }
}
