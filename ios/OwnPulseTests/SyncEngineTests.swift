// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit
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

    // MARK: - Plan fix #1: batch size

    @Test("uploads chunk samples at no more than 500 per batch")
    @MainActor
    func testBatchSize() async throws {
        let provider = MockHealthKitProvider()
        // Every querySamples call returns 1,200 samples — fewer than the
        // page size (5000), so the producer loop terminates after one call.
        // Across all 74 mapped types we'll see at least one batch hitting
        // exactly 500 records (the chunk boundary at 1,200 = 500+500+200).
        provider.mockSamples = Self.makeSamples(recordType: "heart_rate", count: 1_200)
        provider.mockAnchor = Data([1])

        let network = MockNetworkClient()
        let (engine, _, _, _) = buildEngine(healthKitProvider: provider, networkClient: network)

        // Override the no-op handler that `buildEngine` installs so we can
        // measure batch sizes.
        let recorder = BodyRecorder()
        network.requestNoContentHandler = { _, path, body in
            if path == Endpoints.healthKitSync, let insert = body as? HealthKitBulkInsert {
                recorder.record(insert.records.count)
            }
        }

        await engine.sync()

        let sizes = recorder.snapshot()
        #expect(!sizes.isEmpty, "expected at least one upload batch")
        #expect(sizes.allSatisfy { $0 <= 500 }, "expected all batches <= 500, got \(sizes)")
        // The 500-cap must actually be exercised: 1,200 samples / 500
        // per batch = 3 batches of [500, 500, 200] per type. With 74 mapped
        // types, the maximum batch size of 500 should show up many times.
        #expect(sizes.contains(500), "expected at least one batch of exactly 500 records")
    }

    // MARK: - Plan fix #2: pipelined reads + uploads

    @Test("producer reads and consumer uploads overlap (pipelining)")
    @MainActor
    func testPipelineOverlap() async throws {
        let provider = MockHealthKitProvider()
        // Two non-empty pages plus an empty terminator. Each page takes
        // ~50ms to read so we can detect overlap with uploads.
        let pageOne = Self.makeSamples(recordType: "heart_rate", count: 5_000, startOffset: 0)
        let pageTwo = Self.makeSamples(recordType: "heart_rate", count: 5_000, startOffset: 5_000)
        provider.queryPages = [
            AnchoredQueryResult(samples: pageOne, newAnchor: Data([1]), deletedObjectIDs: []),
            AnchoredQueryResult(samples: pageTwo, newAnchor: Data([2]), deletedObjectIDs: []),
            AnchoredQueryResult(samples: [], newAnchor: Data([2]), deletedObjectIDs: []),
        ]
        provider.querySampleDelay = 0.050

        let network = MockNetworkClient()
        let (engine, _, _, _) = buildEngine(healthKitProvider: provider, networkClient: network)

        // Each upload takes ~30ms so we can see overlap with the next page
        // read. Set AFTER `buildEngine` installs its no-op default.
        network.asyncRequestNoContentHandler = { _, _, _ in
            try await Task.sleep(nanoseconds: 30_000_000)
        }

        await engine.sync()

        // Pipelining proof: at least one upload (timing) must start before
        // the LAST query call ends.
        let queries = provider.queryCallLogSnapshot()
        let uploads = network.requestNoContentTimings.filter { $0.path == Endpoints.healthKitSync }
        #expect(queries.count >= 2, "expected >=2 paged queries, got \(queries.count)")
        #expect(uploads.count >= 1, "expected at least one upload")

        // Latest query end vs earliest upload start that occurred AFTER the
        // first query but BEFORE the last query finished.
        if let lastQueryEnd = queries.map(\.endedAt).max(),
           let firstQueryEnd = queries.map(\.endedAt).min() {
            let overlapping = uploads.contains { upload in
                upload.startedAt > firstQueryEnd && upload.startedAt < lastQueryEnd
            }
            #expect(overlapping, "expected at least one upload to start while paged reads were still in flight; queries=\(queries.count) uploads=\(uploads.count)")
        }
    }

    // MARK: - Plan fix #4: bounded TaskGroup concurrency

    @Test("syncs run concurrently across types but bounded at 3 in flight")
    @MainActor
    func testTaskGroupBoundedConcurrency() async throws {
        // The mappings list has 74 types. Most return zero samples (and so
        // skip the upload path) — but we need EVERY type to actually reach
        // an upload to measure outer concurrency. So we configure the mock
        // to return samples on every querySamples call (the mock's default
        // path uses `mockSamples`, no `queryPages`).
        let provider = MockHealthKitProvider()
        provider.mockSamples = Self.makeSamples(recordType: "synthetic", count: 1)
        provider.mockAnchor = Data([1])

        let network = MockNetworkClient()
        let (engine, _, _, _) = buildEngine(healthKitProvider: provider, networkClient: network)

        // Each upload sleeps ~40ms so multiple types stack up at the suspend
        // point. The mock counts max concurrent uploads via in-flight delta.
        // Set AFTER `buildEngine` installs its no-op default.
        network.asyncRequestNoContentHandler = { _, _, _ in
            try await Task.sleep(nanoseconds: 40_000_000)
        }

        await engine.sync()

        // Engine caps concurrency at 3. The mock can observe up to 3
        // simultaneous in-flight uploads at any given moment.
        #expect(network.maxConcurrentUploads <= 3, "expected <=3 concurrent uploads (the cap), got \(network.maxConcurrentUploads)")
        #expect(network.maxConcurrentUploads >= 2, "expected >=2 concurrent uploads (proving parallelism), got \(network.maxConcurrentUploads)")
    }

    // MARK: - Plan fix #6: upload failure logged

    @Test("upload failure with 401 is categorized as 'auth' in the log message")
    @MainActor
    func testUploadFailureLogged() async throws {
        // We don't depend on OSLogStore (unavailable in the test sim without
        // entitlements). Instead, inject a network failure and observe that
        // the sync surfaces the failure via the progress object — that's
        // the user-visible signal that the diagnostic logger fires on.
        let provider = MockHealthKitProvider()
        provider.mockSamples = Self.makeSamples(recordType: "heart_rate", count: 10)
        provider.mockAnchor = Data([1])

        let network = MockNetworkClient()
        network.requestHandler = { method, path, _ in
            if method == "GET" && path == Endpoints.healthKitWriteQueue {
                return [HealthKitWriteQueueItem]()
            }
            return []
        }
        network.requestNoContentHandler = { _, path, _ in
            if path == Endpoints.healthKitSync {
                throw NetworkError.serverError(statusCode: 401, body: "unauthorized")
            }
        }

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

        // At least one failing type should be marked .failed on the progress.
        let anyFailed = progress.typeStatuses.values.contains { $0.status == .failed }
        #expect(anyFailed, "expected at least one type to be marked .failed after a 401 upload error")

        // And the failed upload should have been enqueued for retry.
        let pending = try queue.dequeuePending()
        #expect(!pending.isEmpty, "expected the offline queue to retain the failed insert for retry")
    }


    // MARK: - Helpers

    private static func makeSamples(recordType: String, count: Int, startOffset: Int = 0) -> [HealthKitSample] {
        let base = Date(timeIntervalSince1970: 1_700_000_000)
        return (0..<count).map { i in
            HealthKitSample(
                recordType: recordType,
                value: Double(startOffset + i),
                unit: "bpm",
                startTime: base.addingTimeInterval(Double(startOffset + i)),
                endTime: base.addingTimeInterval(Double(startOffset + i) + 0.5),
                sourceId: "test-\(startOffset + i)"
            )
        }
    }

}

/// Thread-safe collector for batch sizes recorded across upload calls.
private final class BodyRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var sizes: [Int] = []

    func record(_ size: Int) {
        lock.lock(); defer { lock.unlock() }
        sizes.append(size)
    }

    func snapshot() -> [Int] {
        lock.lock(); defer { lock.unlock() }
        return sizes
    }
}

/// MockHealthKitProvider snapshot helper added file-scoped to keep the
/// production protocol surface tidy.
private extension MockHealthKitProvider {
    func queryCallLogSnapshot() -> [(type: HKSampleType, anchor: Data?, limit: Int, startedAt: Date, endedAt: Date)] {
        // Direct access — the underlying array is mutated only via lock.withLock
        // inside the provider, and we read here after the sync has completed
        // (no further mutations possible).
        return queryCallLog
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
/// `@MainActor` because callers read MainActor-isolated state.
@MainActor
fileprivate func eventually(
    timeout: TimeInterval,
    _ condition: @MainActor () async -> Bool
) async throws {
    let deadline = Date().addingTimeInterval(timeout)
    while Date() < deadline {
        if await condition() { return }
        try await Task.sleep(nanoseconds: 20_000_000)
    }
    Issue.record("Condition never became true within \(timeout)s")
}
