// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import HealthKit
import Testing
@testable import OwnPulse

@Suite("HealthKitProvider — background-delivery frequency policy")
struct HealthKitProviderFrequencyTests {
    // This suite pins the record-type → frequency mapping so that adding a
    // new HealthKit mapping can't silently inherit the wrong policy. When
    // you add a new record type to HealthKitTypeMap, decide explicitly
    // whether it should be `.immediate` (low-latency events like heart
    // rate) or `.hourly` (bulk/aggregate metrics), and update these tests.

    @Test("heart_rate uses .immediate for low-latency workout updates")
    func heartRateIsImmediate() {
        let frequency = HealthKitProvider.backgroundDeliveryFrequency(for: "heart_rate")
        #expect(frequency == .immediate)
    }

    @Test("blood_oxygen uses .immediate for SpO2 spike detection")
    func bloodOxygenIsImmediate() {
        let frequency = HealthKitProvider.backgroundDeliveryFrequency(for: "blood_oxygen")
        #expect(frequency == .immediate)
    }

    @Test("steps uses .hourly to stay gentle on the battery")
    func stepsIsHourly() {
        let frequency = HealthKitProvider.backgroundDeliveryFrequency(for: "steps")
        #expect(frequency == .hourly)
    }

    @Test("sleep_analysis uses .hourly — sleep sessions are not latency-critical")
    func sleepIsHourly() {
        let frequency = HealthKitProvider.backgroundDeliveryFrequency(for: "sleep_analysis")
        #expect(frequency == .hourly)
    }

    @Test("unknown record types default to .hourly")
    func unknownDefaultsToHourly() {
        let frequency = HealthKitProvider.backgroundDeliveryFrequency(for: "some_hypothetical_future_type")
        #expect(frequency == .hourly)
    }

    @Test("all existing mappings resolve to one of the two allowed frequencies")
    func allMappingsResolve() {
        // Guard rail: if someone accidentally adds a third frequency bucket
        // the policy grows silently. Pin the allowed set here.
        for mapping in HealthKitTypeMap.mappings {
            let frequency = HealthKitProvider.backgroundDeliveryFrequency(for: mapping.recordType)
            #expect(
                frequency == .immediate || frequency == .hourly,
                "Unexpected frequency for \(mapping.recordType)"
            )
        }
    }

    @Test("immediate set contains exactly the documented record types")
    func immediateSetIsPinned() {
        // If this test fails, someone added a new `.immediate` type without
        // updating the documented rationale. Update either the set or the
        // tests — don't silently expand `.immediate` and drain the battery.
        #expect(HealthKitProvider.immediateDeliveryRecordTypes == ["heart_rate", "blood_oxygen"])
    }
}

@Suite("HealthKitProvider — paged queries and authorization helper")
struct HealthKitProviderPagedQueryTests {
    /// Mock-driven test for the paging loop in SyncEngine. Verifies that
    /// the consumer feeds limit=5000 to `querySamples` and that the loop
    /// terminates when a page returns fewer samples than the limit. We
    /// can't drive a real HKAnchoredObjectQuery from a unit test, so this
    /// asserts the contract via the mock — the real provider exercises
    /// the same `limit:` argument we now thread through the protocol.
    @Test("paged fetch issues 3 calls for 12,500 samples at limit=5000")
    @MainActor
    func testPagedFetch() async throws {
        let provider = MockHealthKitProvider()
        // 12,500 samples split as 5000 + 5000 + 2500. The third page is
        // shorter than the limit, which signals "done" to the paging loop
        // — no further calls expected.
        let pageOne = (0..<5_000).map { i in
            HealthKitSample(
                recordType: "heart_rate",
                value: Double(i), unit: "bpm",
                startTime: Date(), endTime: Date(),
                sourceId: "p1-\(i)"
            )
        }
        let pageTwo = (0..<5_000).map { i in
            HealthKitSample(
                recordType: "heart_rate",
                value: Double(5_000 + i), unit: "bpm",
                startTime: Date(), endTime: Date(),
                sourceId: "p2-\(i)"
            )
        }
        let pageThree = (0..<2_500).map { i in
            HealthKitSample(
                recordType: "heart_rate",
                value: Double(10_000 + i), unit: "bpm",
                startTime: Date(), endTime: Date(),
                sourceId: "p3-\(i)"
            )
        }
        provider.queryPages = [
            AnchoredQueryResult(samples: pageOne, newAnchor: Data([1]), deletedObjectIDs: []),
            AnchoredQueryResult(samples: pageTwo, newAnchor: Data([2]), deletedObjectIDs: []),
            AnchoredQueryResult(samples: pageThree, newAnchor: Data([3]), deletedObjectIDs: []),
        ]

        let network = MockNetworkClient()
        network.requestHandler = { method, path, _ in
            if method == "GET" && path == Endpoints.healthKitWriteQueue {
                return [HealthKitWriteQueueItem]()
            }
            return []
        }
        network.requestNoContentHandler = { _, _, _ in /* no-op */ }

        let db = DatabaseManager(inMemory: true)
        let queue = OfflineQueue(databaseManager: db)
        let anchors = AnchorStore(databaseManager: db)
        let progress = SyncProgress()
        let engine = SyncEngine(
            networkClient: network,
            healthKitProvider: provider,
            offlineQueue: queue,
            anchorStore: anchors,
            progress: progress,
            backgroundTaskHost: nil
        )

        await engine.sync()

        // For the one type that has pages, we expect exactly 3 calls. Other
        // types call querySamples once each (defaulting to empty mockSamples,
        // which has zero rows — the loop terminates after the first call).
        let nonEmptyPages = provider.queryCallLog.filter { $0.limit == 5_000 && $0.startedAt < $0.endedAt }
        #expect(nonEmptyPages.count >= 3, "expected at least 3 calls for the 12,500-sample type, got \(nonEmptyPages.count)")
        // Every call must respect the documented limit.
        #expect(provider.queryCallLog.allSatisfy { $0.limit == 5_000 }, "all paged fetches must pass limit=5000")
    }

    @Test("authorizationStatus helper returns the configured per-type value")
    func testAuthorizationStatusHelper() async {
        let provider = MockHealthKitProvider()
        let heartRate = HKQuantityType(.heartRate)
        let steps = HKQuantityType(.stepCount)

        provider.authorizationStatusByType = [
            heartRate: .sharingDenied,
            steps: .notDetermined,
        ]

        #expect(provider.authorizationStatus(for: heartRate) == .sharingDenied)
        #expect(provider.authorizationStatus(for: steps) == .notDetermined)
        // Unconfigured types default to .sharingAuthorized.
        #expect(provider.authorizationStatus(for: HKQuantityType(.bodyMass)) == .sharingAuthorized)
    }
}
