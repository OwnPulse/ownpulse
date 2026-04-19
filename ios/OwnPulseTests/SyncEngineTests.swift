// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("SyncEngine")
struct SyncEngineTests {
    // MARK: - Helpers

    @MainActor
    private func buildEngine(
        healthKitProvider: MockHealthKitProvider = MockHealthKitProvider(),
        networkClient: MockNetworkClient = MockNetworkClient(),
        backgroundTaskHost: RecordingBackgroundTaskHost? = nil
    ) -> (
        engine: SyncEngine,
        network: MockNetworkClient,
        provider: MockHealthKitProvider,
        host: RecordingBackgroundTaskHost?
    ) {
        let db = DatabaseManager(inMemory: true)
        let queue = OfflineQueue(databaseManager: db)
        let anchors = AnchorStore(databaseManager: db)
        let progress = SyncProgress()

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
            progress: progress,
            backgroundTaskHost: backgroundTaskHost
        )

        return (engine, networkClient, healthKitProvider, backgroundTaskHost)
    }

    // MARK: - Background task wrapping

    @Test("sync() begins and ends a background task through the host")
    @MainActor
    func syncWrapsInBackgroundTask() async {
        let host = RecordingBackgroundTaskHost()
        let (engine, _, _, _) = buildEngine(backgroundTaskHost: host)

        await engine.sync()

        // Give the defer-scheduled `end` task a moment to run. It's launched
        // on an unstructured Task; yielding once should be enough.
        await Task.yield()
        try? await Task.sleep(nanoseconds: 50_000_000)

        #expect(host.beginCallCount == 1)
        #expect(host.endCallCount == 1)
        #expect(host.beginNames == ["healthkit-sync"])
    }

    @Test("sync() ends the background task even when the inner operation fails")
    @MainActor
    func endsTaskOnError() async {
        let host = RecordingBackgroundTaskHost()
        let (engine, network, _, _) = buildEngine(backgroundTaskHost: host)

        // Override the write-back GET to throw, ensuring the sync hits an
        // error branch. The `defer` block must still end the task.
        network.requestHandler = { method, path, _ in
            if method == "GET" && path == Endpoints.healthKitWriteQueue {
                throw NetworkError.serverError(statusCode: 500, body: "boom")
            }
            return []
        }

        await engine.sync()
        await Task.yield()
        try? await Task.sleep(nanoseconds: 50_000_000)

        #expect(host.beginCallCount == 1)
        #expect(host.endCallCount == 1)
    }

    @Test("sync() is re-entrant-safe — parallel calls do not double-begin")
    @MainActor
    func reentrancyGuard() async {
        let host = RecordingBackgroundTaskHost()
        let (engine, _, _, _) = buildEngine(backgroundTaskHost: host)

        // Fire two syncs simultaneously. The inner `guard !_isSyncing` should
        // drop the second one before it begins a background task.
        async let first: () = engine.sync()
        async let second: () = engine.sync()
        _ = await (first, second)

        await Task.yield()
        try? await Task.sleep(nanoseconds: 50_000_000)

        #expect(host.beginCallCount == 1)
        #expect(host.endCallCount == 1)
    }

    @Test("sync() without a background task host still completes")
    @MainActor
    func worksWithoutHost() async {
        let (engine, _, _, _) = buildEngine(backgroundTaskHost: nil)
        await engine.sync()

        let isSyncing = await engine.isSyncing
        #expect(isSyncing == false)
    }

    @Test("two consecutive syncs produce two matched begin/end pairs")
    @MainActor
    func serialSyncsProduceMatchedPairs() async {
        let host = RecordingBackgroundTaskHost()
        let (engine, _, _, _) = buildEngine(backgroundTaskHost: host)

        await engine.sync()
        try? await Task.sleep(nanoseconds: 50_000_000)
        await engine.sync()
        try? await Task.sleep(nanoseconds: 50_000_000)

        #expect(host.beginCallCount == 2)
        #expect(host.endCallCount == 2)
    }

    // MARK: - Fix #4: TaskHandle expiration-race coverage

    @Test("expiration fired AFTER normal end is a no-op (idempotent-end guard)")
    @MainActor
    func expirationAfterNormalEndIsNoOp() async throws {
        let host = RecordingBackgroundTaskHost()
        let (engine, _, _, _) = buildEngine(backgroundTaskHost: host)

        await engine.sync()
        try await Task.sleep(nanoseconds: 100_000_000)
        #expect(host.beginCallCount == 1)
        #expect(host.endCallCount == 1)

        // Simulate iOS firing the expiration handler AFTER the defer already
        // ended the task. The TaskHandle's `ended` flag must swallow this so
        // we don't end the same id twice (UIApplication logs an error if we
        // do, though it doesn't crash).
        host.triggerExpiration(id: 1)
        try await Task.sleep(nanoseconds: 100_000_000)

        #expect(host.endCallCount == 1)
    }

    @Test("expiration fired DURING the sync ends the task once and defer is a no-op")
    @MainActor
    func expirationDuringSyncEndsTaskOnce() async throws {
        let host = RecordingBackgroundTaskHost()
        let network = MockNetworkClient()

        // Stall the sync at the write-queue GET so the expiration handler
        // fires while the sync is still in flight.
        let gate = Gate()
        network.asyncRequestHandler = { method, path, _ in
            if method == "GET" && path == Endpoints.healthKitWriteQueue {
                await gate.wait()
                return [HealthKitWriteQueueItem]()
            }
            return [] as [HealthKitWriteQueueItem]
        }
        network.requestNoContentHandler = { _, _, _ in /* no-op */ }

        let db = DatabaseManager(inMemory: true)
        let queue = OfflineQueue(databaseManager: db)
        let anchors = AnchorStore(databaseManager: db)
        let progress = SyncProgress()
        let engine = SyncEngine(
            networkClient: network,
            healthKitProvider: MockHealthKitProvider(),
            offlineQueue: queue,
            anchorStore: anchors,
            progress: progress,
            backgroundTaskHost: host
        )

        // Kick off sync — it stalls inside the write-queue GET.
        let syncTask = Task { await engine.sync() }

        // Wait until the task has begun the background task.
        try await eventually(timeout: 2.0) {
            host.beginCallCount == 1
        }
        #expect(host.endCallCount == 0)

        // Fire iOS expiration mid-flight. TaskHandle should end the task.
        host.triggerExpiration(id: 1)
        try await eventually(timeout: 2.0) {
            host.endCallCount == 1
        }

        // Let the sync complete. The `defer` cleanup should see that the
        // TaskHandle is already ended and skip a second end call.
        await gate.open()
        await syncTask.value
        try await Task.sleep(nanoseconds: 100_000_000)

        #expect(host.beginCallCount == 1)
        #expect(host.endCallCount == 1, "defer must not double-end")
    }
}

// MARK: - Recording background task host

/// Records begin/end pairs for assertion. Retains captured expiration
/// handlers even after `endBackgroundTask` runs so tests can still invoke
/// them via `triggerExpiration(id:)` — the production `TaskHandle` must
/// treat a post-end expiration call as a no-op to avoid double-end logs.
final class RecordingBackgroundTaskHost: BackgroundTaskHost, @unchecked Sendable {
    private let lock = NSLock()
    private var _beginCallCount = 0
    private var _endCallCount = 0
    private var _beginNames: [String] = []
    private var _endedIds: [Int] = []
    private var nextId = 1
    /// Captured expiration handlers indexed by task id. Retained for the
    /// lifetime of the host so post-end fires are observable.
    private var handlers: [Int: @Sendable () -> Void] = [:]

    var beginCallCount: Int { lock.lock(); defer { lock.unlock() }; return _beginCallCount }
    var endCallCount: Int { lock.lock(); defer { lock.unlock() }; return _endCallCount }
    var beginNames: [String] { lock.lock(); defer { lock.unlock() }; return _beginNames }
    var endedIds: [Int] { lock.lock(); defer { lock.unlock() }; return _endedIds }

    @MainActor
    func beginBackgroundTask(
        name: String,
        expirationHandler: @escaping @Sendable () -> Void
    ) -> Int {
        lock.lock(); defer { lock.unlock() }
        _beginCallCount += 1
        _beginNames.append(name)
        let id = nextId
        nextId += 1
        handlers[id] = expirationHandler
        return id
    }

    @MainActor
    func endBackgroundTask(_ id: Int) {
        lock.lock(); defer { lock.unlock() }
        _endCallCount += 1
        _endedIds.append(id)
        // Note: we do NOT remove the handler — the test driver still wants
        // to fire it to verify the TaskHandle's idempotent-end guard.
    }

    /// Invoke the expiration handler captured for the given task id.
    /// Tests use this to simulate iOS running out of background runway.
    func triggerExpiration(id: Int) {
        lock.lock()
        let handler = handlers[id]
        lock.unlock()
        handler?()
    }
}

/// File-scoped copy of the async gate in SyncCoordinatorTests. Swift allows
/// two files to each have their own `private`/`fileprivate` helper with the
/// same name without a link-time conflict.
fileprivate actor Gate {
    private var isOpen = false
    private var waiters: [CheckedContinuation<Void, Never>] = []

    func wait() async {
        if isOpen { return }
        await withCheckedContinuation { (cont: CheckedContinuation<Void, Never>) in
            waiters.append(cont)
        }
    }

    func open() {
        isOpen = true
        let pending = waiters
        waiters.removeAll()
        for w in pending {
            w.resume()
        }
    }
}

/// Polls `condition` up to `timeout` seconds, sleeping 20ms between checks.
fileprivate func eventually(
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
