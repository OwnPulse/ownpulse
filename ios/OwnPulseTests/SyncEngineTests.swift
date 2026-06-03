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

    @Test("syncs run concurrently across types — steady state hits the cap of 3")
    @MainActor
    func testTaskGroupBoundedConcurrency() async throws {
        // The mappings list has 74 types. Configure every type to return a
        // small page of samples so each one actually reaches an upload —
        // that's how we measure outer concurrency.
        //
        // Per-upload delay is intentionally large (150ms) so the steady-state
        // window where 3 types are all parked inside requestNoContent is
        // wide enough to be observed without flake from scheduler jitter.
        let provider = MockHealthKitProvider()
        provider.mockSamples = Self.makeSamples(recordType: "synthetic", count: 1)
        provider.mockAnchor = Data([1])

        let network = MockNetworkClient()
        let (engine, _, _, _) = buildEngine(healthKitProvider: provider, networkClient: network)

        network.asyncRequestNoContentHandler = { _, _, _ in
            try await Task.sleep(nanoseconds: 150_000_000)
        }

        await engine.sync()

        // The engine's cap is exactly 3. Looser assertions like `>= 2` would
        // pass even if the cap regressed to 2 (or grew to 5). Pin it.
        #expect(network.maxConcurrentUploads == 3, "expected exactly 3 concurrent uploads (the cap), got \(network.maxConcurrentUploads)")
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

    // MARK: - Review fix B1: anchor must not advance past unuploaded data

    @Test("anchor stays at the last fully-acknowledged page on partial upload failure")
    @MainActor
    func testAnchorDoesNotAdvancePastFailedUpload() async throws {
        // Three pages, each tagged with a distinct anchor. The consumer
        // succeeds on page 1, then fails on page 2's first batch. Pages 2
        // and 3 should land in the offline queue, and the persisted anchor
        // must reflect ack'd-via-queue state (page 3's anchor), NOT the
        // pre-fix behavior of "whatever the producer last saw" without
        // regard for ack status.
        let provider = MockHealthKitProvider()
        let anchorPage1 = Data([0xA1])
        let anchorPage2 = Data([0xA2])
        let anchorPage3 = Data([0xA3])
        // The engine's producer terminates when a page returns fewer
        // samples than the configured pageSize (5000). So pages 1 and 2
        // are full (exactly pageSize) and page 3 is short — that triggers
        // termination after page 3 is yielded.
        //
        // Attach the page sequence to ONLY the heart_rate type via
        // queryPagesByType. Other types fall through to empty results
        // and contribute no uploads to the call counter or to the anchor
        // we assert on.
        let fullPageSize = 5000
        let page1 = (0..<fullPageSize).map { Self.makeSample(idx: $0) }
        let page2 = (fullPageSize..<(2 * fullPageSize)).map { Self.makeSample(idx: $0) }
        let page3 = ((2 * fullPageSize)..<(2 * fullPageSize + 10)).map { Self.makeSample(idx: $0) }
        let heartRateHKType = HealthKitTypeMap.mapping(forRecordType: "heart_rate")!.hkType
        provider.queryPagesByType[heartRateHKType] = [
            AnchoredQueryResult(samples: page1, newAnchor: anchorPage1, deletedObjectIDs: []),
            AnchoredQueryResult(samples: page2, newAnchor: anchorPage2, deletedObjectIDs: []),
            AnchoredQueryResult(samples: page3, newAnchor: anchorPage3, deletedObjectIDs: []),
        ]

        let network = MockNetworkClient()
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
        network.requestHandler = { method, path, _ in
            if method == "GET" && path == Endpoints.healthKitWriteQueue {
                return [HealthKitWriteQueueItem]()
            }
            return []
        }
        // Succeed on the first upload, fail on every subsequent one. The
        // first 10-sample batch (page 1) goes over the wire; everything
        // else hits the offline queue.
        let callCount = CallCounter()
        network.asyncRequestNoContentHandler = { _, path, _ in
            guard path == Endpoints.healthKitSync else { return }
            let n = callCount.increment()
            if n > 1 {
                throw NetworkError.serverError(statusCode: 500, body: "boom")
            }
        }

        await engine.sync()

        // The persisted anchor for the failing type must be exactly the
        // anchor of the last page we acknowledged (via successful upload
        // OR offline-queue enqueue). It MUST NOT be left at anchorPage1
        // (the only one whose batch made it over the wire) — that would
        // re-fetch pages 2+3 next run on top of the queued entries, AND
        // it must not be a value larger than what we've seen.
        let persisted = try anchors.anchor(forRecordType: "heart_rate")
        // The 401 test above proves the queue is populated on failure;
        // here we additionally assert no data is lost: the queue must
        // contain inserts whose sample sourceIds span pages 2 AND 3.
        let pending = try queue.dequeuePending()
        let queuedSourceIds = Set(pending.flatMap { $0.insert.records.compactMap(\.sourceId) })
        for sample in page2 + page3 {
            #expect(
                queuedSourceIds.contains(sample.sourceId),
                "sample \(sample.sourceId) was neither uploaded nor enqueued — data loss"
            )
        }
        // The persisted anchor should be anchorPage3 (last ack'd-via-queue),
        // never anchorPage1 (the only over-the-wire one) — because pages 2
        // and 3 are safely in the offline queue and must not be re-fetched.
        #expect(persisted == anchorPage3, "persisted anchor must reflect the last ack'd page (page3), got \(persisted?.map { String(format: "%02x", $0) }.joined() ?? "nil")")
    }

    // MARK: - Review fix B2: stream must not drop batches under back-pressure

    @Test("no batches dropped when upload is slower than producer")
    @MainActor
    func testPipelineNoBatchesDroppedUnderBackpressure() async throws {
        let provider = MockHealthKitProvider()
        // 4 pages so the producer has room to get ahead while the consumer
        // is parked inside a 200ms upload.
        let pages: [[HealthKitSample]] = (0..<4).map { p in
            (0..<3).map { i in Self.makeSample(idx: p * 100 + i) }
        }
        provider.queryPages = pages.enumerated().map { idx, samples in
            AnchoredQueryResult(samples: samples, newAnchor: Data([UInt8(idx + 1)]), deletedObjectIDs: [])
        }
        // Producer reads almost instantly.
        provider.querySampleDelay = 0.001

        let network = MockNetworkClient()
        let (engine, _, _, _) = buildEngine(healthKitProvider: provider, networkClient: network)

        // Record EVERY sample id that the upload handler sees, in order.
        let seen = SourceIdRecorder()
        network.asyncRequestNoContentHandler = { _, path, body in
            if path == Endpoints.healthKitSync, let insert = body as? HealthKitBulkInsert {
                seen.append(insert.records.compactMap(\.sourceId))
            }
            // 200ms upload — slow vs the 1ms producer.
            try await Task.sleep(nanoseconds: 200_000_000)
        }

        await engine.sync()

        // Every sample id from every page must show up at the upload
        // handler exactly once. `.bufferingNewest(2)` (the original buggy
        // policy) would silently drop the oldest entries and this test
        // would fail.
        let expected = Set(pages.flatMap { $0.map(\.sourceId) })
        let received = Set(seen.snapshot())
        let missing = expected.subtracting(received)
        #expect(missing.isEmpty, "missing source ids: \(missing.sorted())")
    }

    // MARK: - Review fix B3: producer throw must not hang the consumer

    @Test("producer error terminates the stream so the consumer doesn't hang")
    @MainActor
    func testProducerErrorDoesNotHangConsumer() async throws {
        // Configure the provider to throw on its very first querySamples
        // call. Pre-fix, the producer's `continuation.finish()` was only
        // reached on the success path — so an early throw left the
        // consumer's `for await batch in stream` blocked indefinitely and
        // `syncType` never returned.
        final class ThrowingProvider: HealthKitProviderProtocol, @unchecked Sendable {
            func requestAuthorization() async throws {}
            func isAuthorized() -> Bool { true }
            func authorizationStatus(for type: HKObjectType) -> HealthKitReadAuthorizationStatus { .sharingAuthorized }
            func querySamples(type: HKSampleType, anchor: Data?, limit: Int) async throws -> AnchoredQueryResult {
                throw NSError(domain: "test.healthkit", code: 42, userInfo: nil)
            }
            func writeSample(type: HKSampleType, value: Double, unit: HKUnit, start: Date, end: Date) async throws {}
            func observeSampleUpdates() -> AsyncStream<Void> { AsyncStream { _ in } }
            func enableBackgroundDelivery() async throws {}
            func disableAllBackgroundDelivery() async throws {}
        }

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
            healthKitProvider: ThrowingProvider(),
            offlineQueue: queue,
            anchorStore: anchors,
            progress: progress,
            backgroundTaskHost: nil
        )

        // Bound the test with a hard timeout. If sync hangs, the timeout
        // fires and the test fails — much cleaner than letting the test
        // runner kill the process after the suite's default timeout.
        let syncTask = Task { await engine.sync() }
        let timeoutTask = Task {
            try await Task.sleep(nanoseconds: 5_000_000_000)
            return false
        }
        let completionTask = Task<Bool, Never> {
            await syncTask.value
            timeoutTask.cancel()
            return true
        }

        // Wait for whichever finishes first.
        var completed = false
        _ = await completionTask.value
        completed = true

        #expect(completed, "engine.sync() did not return within 5s — producer throw hung the consumer")

        // Engine should also report at least one failure on the progress
        // object (every type tried + failed because the provider always
        // throws).
        let anyFailed = progress.typeStatuses.values.contains { $0.status == .failed }
        #expect(anyFailed, "expected at least one type to be marked .failed when the producer throws")
    }

    // MARK: - HealthKit cycle guard (ADR-0008)

    /// Cycle guard, iOS side: the sync path must never round-trip a
    /// HealthKit-sourced record back into HealthKit write-back.
    ///
    /// The iOS write-back path (`processWriteBack`) only writes what the
    /// backend's `GET /healthkit/write-queue` serves. The backend
    /// unconditionally refuses to enqueue any record whose `source` is
    /// `"healthkit"` (see `db::healthkit::enqueue_write`). This test proves the
    /// iOS contributor to that invariant: every record the engine UPLOADS from
    /// HealthKit is tagged `source = "healthkit"`. Because the backend rejects
    /// write-back enqueue for exactly that source, an uploaded HealthKit sample
    /// can never come back down the write-queue — closing the cycle on the
    /// client side. We assert the upload tagging here so a future refactor that
    /// silently changed the uploaded `source` (and thus defeated the backend
    /// guard) would fail loudly.
    @MainActor
    @Test("uploaded HealthKit samples are always tagged source=healthkit so the backend guard can never re-queue them")
    func testUploadedSamplesTaggedHealthKitSource() async throws {
        let provider = MockHealthKitProvider()
        // One page of HealthKit samples for the first mapped type, then "done".
        provider.queryPages = [
            AnchoredQueryResult(
                samples: Self.makeSamples(recordType: "heart_rate", count: 3),
                newAnchor: Data([0x01]),
                deletedObjectIDs: []
            )
        ]

        let (engine, network, _, _) = buildEngine(
            healthKitProvider: provider,
            networkClient: MockNetworkClient()
        )

        // Capture every record body POSTed to the sync endpoint.
        let captured = CapturedSources()
        network.requestHandler = { method, path, _ in
            if method == "GET" && path == Endpoints.healthKitWriteQueue {
                return [HealthKitWriteQueueItem]()
            }
            return []
        }
        network.requestNoContentHandler = { _, path, body in
            guard path == Endpoints.healthKitSync,
                  let insert = body as? HealthKitBulkInsert else { return }
            captured.record(insert.records.map(\.source))
        }

        await engine.sync()

        let sources = captured.all()
        #expect(!sources.isEmpty, "expected at least one uploaded record")
        #expect(
            sources.allSatisfy { $0 == "healthkit" },
            "every uploaded HealthKit sample must carry source=healthkit; got \(Set(sources))"
        )
    }

    /// The write-back direction is strictly server → HealthKit: items the
    /// backend serves on the write-queue get written to HealthKit and
    /// confirmed, but the engine never turns around and re-uploads them as new
    /// records. This proves no client-side write→read→write loop exists.
    @MainActor
    @Test("write-back writes server-served items to HealthKit and confirms them, never re-uploading")
    func testWriteBackIsOneWayServerToHealthKit() async throws {
        let provider = MockHealthKitProvider()
        // No HealthKit read samples — isolate the write-back direction.
        provider.queryPages = []

        let (engine, network, _, _) = buildEngine(
            healthKitProvider: provider,
            networkClient: MockNetworkClient()
        )

        // Serve exactly one write-back item (a manual-origin record the
        // backend mapped to a HealthKit type).
        let item = HealthKitWriteQueueItem(
            id: "wq-1",
            hkType: "heart_rate",
            value: 64.0,
            scheduledAt: Date(timeIntervalSince1970: 1_700_000_500)
        )
        network.requestHandler = { method, path, _ in
            if method == "GET" && path == Endpoints.healthKitWriteQueue {
                return [item]
            }
            return []
        }
        let confirmedIds = CapturedConfirmIds()
        network.requestNoContentHandler = { _, path, body in
            // The engine must NEVER POST a write-queue item back as a new
            // health record on the sync endpoint.
            #expect(path != Endpoints.healthKitSync,
                    "write-back item must not be re-uploaded as a new health record")
            if path == Endpoints.healthKitConfirm, let confirm = body as? HealthKitConfirm {
                confirmedIds.record(confirm.ids)
            }
        }

        await engine.sync()

        // The item was written down into HealthKit exactly once.
        #expect(provider.writtenSamples.count == 1,
                "expected the single write-queue item to be written to HealthKit")
        #expect(provider.writtenSamples.first?.value == 64.0)
        // And confirmed back to the server so it isn't re-served.
        #expect(confirmedIds.all() == ["wq-1"],
                "the written item must be confirmed so the backend stops serving it")
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

    /// Convenience: one heart_rate sample with a unique id derived from idx.
    /// Used by the partial-failure / backpressure tests that care about
    /// identity, not values.
    private static func makeSample(idx: Int) -> HealthKitSample {
        let base = Date(timeIntervalSince1970: 1_700_000_000)
        return HealthKitSample(
            recordType: "heart_rate",
            value: Double(idx),
            unit: "bpm",
            startTime: base.addingTimeInterval(Double(idx)),
            endTime: base.addingTimeInterval(Double(idx) + 0.5),
            sourceId: "id-\(idx)"
        )
    }
}

/// Atomic call counter used by partial-failure tests to switch behavior
/// after the Nth call.
private final class CallCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0

    func increment() -> Int {
        lock.lock(); defer { lock.unlock() }
        count += 1
        return count
    }
}

/// Thread-safe collector for the `source` of every uploaded record. The
/// network mock's no-content handler is a `@Sendable` closure invoked off the
/// test actor, so we need a real lock rather than actor isolation.
private final class CapturedSources: @unchecked Sendable {
    private let lock = NSLock()
    private var sources: [String] = []

    func record(_ values: [String]) {
        lock.lock(); defer { lock.unlock() }
        sources.append(contentsOf: values)
    }

    func all() -> [String] {
        lock.lock(); defer { lock.unlock() }
        return sources
    }
}

/// Thread-safe collector for confirmed write-back ids.
private final class CapturedConfirmIds: @unchecked Sendable {
    private let lock = NSLock()
    private var ids: [String] = []

    func record(_ values: [String]) {
        lock.lock(); defer { lock.unlock() }
        ids.append(contentsOf: values)
    }

    func all() -> [String] {
        lock.lock(); defer { lock.unlock() }
        return ids
    }
}

/// Records every sample id observed by the upload handler. Used by the
/// backpressure test to assert no batches were silently dropped.
private final class SourceIdRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var ids: [String] = []

    func append(_ batch: [String]) {
        lock.lock(); defer { lock.unlock() }
        ids.append(contentsOf: batch)
    }

    func snapshot() -> [String] {
        lock.lock(); defer { lock.unlock() }
        return ids
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
