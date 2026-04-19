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
}

// MARK: - Recording background task host

/// Records begin/end pairs for assertion. Also surfaces the expiration
/// handler so tests can simulate iOS cutting us off mid-sync.
final class RecordingBackgroundTaskHost: BackgroundTaskHost, @unchecked Sendable {
    private let lock = NSLock()
    private var _beginCallCount = 0
    private var _endCallCount = 0
    private var _beginNames: [String] = []
    private var _endedIds: [Int] = []
    private var nextId = 1
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
        handlers.removeValue(forKey: id)
    }
}
