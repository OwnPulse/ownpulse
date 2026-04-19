// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("SyncCoordinator")
struct SyncCoordinatorTests {
    // MARK: - Helpers

    /// Builds a `SyncEngine` wired to in-memory persistence and a mock
    /// network client that always succeeds. The returned tuple lets the test
    /// observe sync invocations via `networkClient.requestCalls`.
    @MainActor
    private func buildEngine(
        healthKitProvider: MockHealthKitProvider,
        networkClient: MockNetworkClient = MockNetworkClient()
    ) -> (engine: SyncEngine, network: MockNetworkClient) {
        let db = DatabaseManager(inMemory: true)
        let queue = OfflineQueue(databaseManager: db)
        let anchors = AnchorStore(databaseManager: db)
        let progress = SyncProgress()

        // Default handlers: accept all HealthKit sync calls and write-back
        // queue lookups so the sync loop can complete without errors.
        networkClient.requestHandler = { method, path, _ in
            if method == "GET" && path == Endpoints.healthKitWriteQueue {
                return [HealthKitWriteQueueItem]()
            }
            return []
        }
        networkClient.requestNoContentHandler = { _, _, _ in /* no-op */ }

        let engine = SyncEngine(
            networkClient: networkClient,
            healthKitProvider: healthKitProvider,
            offlineQueue: queue,
            anchorStore: anchors,
            progress: progress
        )
        return (engine, networkClient)
    }

    // Sync-call counter — one `GET` to `/healthkit/write-queue` happens per
    // `SyncEngine.sync()` invocation, so counting those requests tells us
    // exactly how many syncs ran.
    private func syncCount(from network: MockNetworkClient) -> Int {
        network.requestCalls.filter {
            $0.method == "GET" && $0.path == Endpoints.healthKitWriteQueue
        }.count
    }

    // MARK: - Tests

    @Test("start() subscribes to the HealthKit observer exactly once")
    @MainActor
    func startSubscribes() async {
        let provider = MockHealthKitProvider()
        let (engine, _) = buildEngine(healthKitProvider: provider)

        let coordinator = SyncCoordinator(
            healthKitProvider: provider,
            syncEngine: engine,
            debounceSeconds: 60
        )

        await coordinator.start()
        #expect(provider.observerStartCount == 1)

        await coordinator.stop()
    }

    @Test("start() is idempotent — calling twice reuses the subscription")
    @MainActor
    func doubleStartIsIdempotent() async {
        let provider = MockHealthKitProvider()
        let (engine, _) = buildEngine(healthKitProvider: provider)

        let coordinator = SyncCoordinator(
            healthKitProvider: provider,
            syncEngine: engine,
            debounceSeconds: 60
        )

        await coordinator.start()
        await coordinator.start()

        #expect(provider.observerStartCount == 1)
        await coordinator.stop()
    }

    @Test("observer event triggers sync after the debounce window")
    @MainActor
    func observerFiresSync() async throws {
        let provider = MockHealthKitProvider()
        let (engine, network) = buildEngine(healthKitProvider: provider)

        // Use a very short debounce so the real `Task.sleep` returns quickly
        // without making the test slow. 50ms is long enough for CI to not
        // race, short enough to keep the suite snappy.
        let coordinator = SyncCoordinator(
            healthKitProvider: provider,
            syncEngine: engine,
            debounceSeconds: 0.05
        )

        await coordinator.start()
        #expect(syncCount(from: network) == 0)

        provider.fireObserver()

        // Poll for sync to complete — more robust than a single sleep.
        try await eventually(timeout: 2.0) {
            syncCount(from: network) == 1
        }

        await coordinator.stop()
    }

    @Test("bursts of observer events coalesce into a single sync")
    @MainActor
    func bursts() async throws {
        let provider = MockHealthKitProvider()
        let (engine, network) = buildEngine(healthKitProvider: provider)

        let coordinator = SyncCoordinator(
            healthKitProvider: provider,
            syncEngine: engine,
            debounceSeconds: 0.15
        )

        await coordinator.start()

        // Fire 10 events at 10ms intervals — faster than the 150ms debounce,
        // so the coordinator should keep resetting its timer and only fire
        // one sync after the burst ends.
        for _ in 0..<10 {
            provider.fireObserver()
            try await Task.sleep(nanoseconds: 10_000_000)
        }

        try await eventually(timeout: 2.0) {
            syncCount(from: network) == 1
        }

        // Give a little extra slack — if the implementation were buggy
        // enough to fire a second sync, it would show up here.
        try await Task.sleep(nanoseconds: 200_000_000)
        #expect(syncCount(from: network) == 1)

        await coordinator.stop()
    }

    @Test("stop() cancels the pending sync so no request is issued")
    @MainActor
    func stopCancelsPendingSync() async throws {
        let provider = MockHealthKitProvider()
        let (engine, network) = buildEngine(healthKitProvider: provider)

        let coordinator = SyncCoordinator(
            healthKitProvider: provider,
            syncEngine: engine,
            debounceSeconds: 1.0 // long enough that stop() fires first
        )

        await coordinator.start()
        provider.fireObserver()

        // Give the observer event a moment to enqueue the debounced task.
        try await Task.sleep(nanoseconds: 50_000_000)

        await coordinator.stop()

        // Wait past what would have been the debounce window — the pending
        // sync should have been cancelled.
        try await Task.sleep(nanoseconds: 1_100_000_000)
        #expect(syncCount(from: network) == 0)
    }
}

/// Polls `condition` up to `timeout` seconds, sleeping 20ms between checks.
/// Fails the test via `Issue.record` if the condition never becomes true.
private func eventually(
    timeout: TimeInterval,
    _ condition: @Sendable () async -> Bool
) async throws {
    let deadline = Date().addingTimeInterval(timeout)
    while Date() < deadline {
        if await condition() { return }
        try await Task.sleep(nanoseconds: 20_000_000)
    }
    Issue.record("Condition never became true within \(timeout)s")
}
