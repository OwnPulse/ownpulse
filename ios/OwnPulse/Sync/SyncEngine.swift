// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit
import Observation
import os

private let engineLogger = Logger(subsystem: "health.ownpulse.app", category: "sync-engine")

// Observable state bag — lives on MainActor, updated by SyncEngine after each operation.
@Observable
@MainActor
final class SyncState {
    private(set) var isSyncing = false
    private(set) var lastSyncDate: Date?
    private(set) var lastError: String?

    func begin() { isSyncing = true; lastError = nil }
    func finish(syncDate: Date?, error: String?) {
        isSyncing = false
        lastSyncDate = syncDate
        lastError = error
    }
}

actor SyncEngine {
    private let networkClient: NetworkClientProtocol
    private let healthKitProvider: HealthKitProviderProtocol
    private let clinicalRecordProvider: ClinicalRecordProviderProtocol?
    private let medicationSyncProvider: (any Sendable)?
    private let offlineQueue: OfflineQueueProtocol
    private let anchorStore: AnchorStore
    private let backgroundTaskHost: BackgroundTaskHost?

    /// Backend caps a single bulk insert at 500 (`MAX_HEALTHKIT_BATCH`).
    /// Match that — fewer round trips, same per-call work.
    private let batchSize = 500

    /// Cap on `HKAnchoredObjectQuery` results per page. With heart rate at
    /// 450K+ samples, materializing the whole array before we can start
    /// uploading is a memory + latency disaster. 5000 keeps peak memory
    /// bounded while still amortizing the cost of HealthKit's IPC.
    private let pageSize = 5000

    /// Max number of types syncing concurrently. The plan calls out 3 as a
    /// deliberate cap — don't compete with the user's foreground requests
    /// and don't hit backend rate limits. Measure before raising.
    private let maxConcurrentTypes = 3

    private var _isSyncing = false
    private var _lastSyncDate: Date?
    private var _lastError: String?

    // Expose to callers that need to await actor-isolated state
    var isSyncing: Bool { _isSyncing }
    var lastSyncDate: Date? { _lastSyncDate }
    var lastError: String? { _lastError }

    private let progress: SyncProgress

    init(
        networkClient: NetworkClientProtocol,
        healthKitProvider: HealthKitProviderProtocol,
        clinicalRecordProvider: ClinicalRecordProviderProtocol? = nil,
        medicationSyncProvider: (any Sendable)? = nil,
        offlineQueue: OfflineQueueProtocol,
        anchorStore: AnchorStore,
        progress: SyncProgress,
        backgroundTaskHost: BackgroundTaskHost? = nil
    ) {
        self.networkClient = networkClient
        self.healthKitProvider = healthKitProvider
        self.clinicalRecordProvider = clinicalRecordProvider
        self.medicationSyncProvider = medicationSyncProvider
        self.offlineQueue = offlineQueue
        self.anchorStore = anchorStore
        self.progress = progress
        self.backgroundTaskHost = backgroundTaskHost
    }

    func sync() async {
        guard !_isSyncing else { return }
        _isSyncing = true
        _lastError = nil

        // Request extra execution time so the sync doesn't stall if the user
        // backgrounds the app mid-upload. iOS suspends foreground processes
        // within seconds of backgrounding; this keeps us alive for up to
        // ~30s (sometimes more) to finish the current batch. If the
        // expiration handler fires we just mark our flag and the next
        // foreground sync picks up where we left off via the anchor store
        // and the offline queue.
        let host = backgroundTaskHost
        let taskHandle = TaskHandle()
        if let host {
            let handle = taskHandle
            let id = await MainActor.run {
                host.beginBackgroundTask(name: "healthkit-sync") {
                    // Expiration handler runs on the main thread. iOS wants to
                    // kill us — end the task promptly so it doesn't terminate
                    // the whole process. `endIfNeeded` is idempotent, so the
                    // later `defer` call becomes a no-op.
                    Task { await handle.endIfNeeded(host: host) }
                }
            }
            await taskHandle.setId(id)
        }

        defer {
            _isSyncing = false
            if let host {
                Task { [taskHandle, host] in
                    await taskHandle.endIfNeeded(host: host)
                }
            }
        }

        do {
            // 1. Drain offline queue first
            try await drainOfflineQueue()

            // 2. Initialize progress tracking — only set up the per-type
            // pending rows on the first sync of a session. Subsequent calls
            // (scene-phase, observer debounce) preserve any in-flight or
            // completed progress so the UI doesn't flicker on re-entry.
            let timestamps = (try? anchorStore.allSyncTimestamps()) ?? [:]
            let types = HealthKitTypeMap.mappings.map {
                (recordType: $0.recordType, displayName: $0.recordType.replacingOccurrences(of: "_", with: " ").capitalized)
            }
            await MainActor.run { progress.prepareIfNeeded(types: types, timestamps: timestamps) }

            // 3. Sync types in parallel, capped at maxConcurrentTypes.
            await runTypeSyncs(HealthKitTypeMap.mappings)
            await MainActor.run { progress.finish() }

            // 4. Sync clinical records (lab results from Apple Health Records) if enabled
            if let clinicalProvider = clinicalRecordProvider,
               ClinicalRecordSettings.isSyncEnabled {
                do {
                    try await syncClinicalRecords(clinicalProvider)
                } catch {
                    // Don't fail the entire sync if clinical records fail
                    _lastError = "Clinical records sync failed: \(error.localizedDescription)"
                }
            }

            // 5. Sync medication dose events (iOS 26+)
            #if swift(>=6.3)
            if #available(iOS 26.0, *) {
                if let provider = medicationSyncProvider as? MedicationSyncProviderProtocol {
                    do {
                        try await syncMedicationDoses(provider)
                    } catch {
                        _lastError = "Medication sync failed: \(error.localizedDescription)"
                    }
                }
            }
            #endif

            // 6. Process write-back queue (non-fatal)
            do {
                try await processWriteBack()
            } catch {
                _lastError = "Write-back failed: \(error.localizedDescription)"
            }

            _lastSyncDate = Date()
        } catch {
            _lastError = error.localizedDescription
        }
    }

    private func drainOfflineQueue() async throws {
        let pending = try offlineQueue.dequeuePending()
        for entry in pending {
            do {
                // POST /healthkit/sync returns a JSON ack body we don't use here.
                // requestNoContent discards it — avoids decoding churn on every retry.
                try await networkClient.requestNoContent(
                    method: "POST",
                    path: Endpoints.healthKitSync,
                    body: entry.insert
                )
                try offlineQueue.markComplete(id: entry.id)
            } catch {
                // Skip and continue — don't let stale queue entries block the entire sync
                logUploadFailure(error, context: "offline-queue-drain")
                _lastError = "Offline queue retry failed: \(error.localizedDescription)"
                // Only a deterministic rejection counts toward abandonment —
                // a transport/connectivity blip is expected to resolve on
                // its own, and the observer debounce can fire this drain
                // every few seconds. Counting every failure would abandon
                // an entry (and eventually the whole queue) during a single
                // brief outage.
                if Self.isDeterministicRejection(error) {
                    try? offlineQueue.recordFailedAttempt(id: entry.id)
                }
            }
        }
        // Surface the current abandoned count so a drop is never silent —
        // read after every drain (not just on new abandonment) so the UI
        // reflects ground truth even if a prior sync abandoned an entry
        // this one didn't touch. This is dedicated, persistent UI state
        // (the `SyncStatusView` warning row) — it must NOT also flow
        // through `_lastError`, which represents THIS sync pass's outcome
        // and would otherwise mark an otherwise-fully-successful sync as
        // failed just because an unrelated, already-known abandonment
        // still exists.
        if let abandoned = try? offlineQueue.abandonedCount() {
            await MainActor.run { self.progress.setAbandonedOfflineEntries(abandoned) }
        }
    }

    /// True for a backend response that will reject the SAME payload
    /// identically forever — the only case where retrying is pointless and
    /// counting toward abandonment is safe. Excludes:
    /// - `.unauthorized` / 401: resolved by the existing token-refresh path.
    /// - 403: `logUploadFailure` already buckets this as "auth", and
    ///   self-hosted reverse proxies commonly emit it for operator-fixable
    ///   reasons (misconfigured auth in front of the API) — not something
    ///   the client can distinguish from a real permanent rejection.
    /// - 408 / 429: transient (timeout / rate limit) by definition.
    /// - anything else (transport errors, 5xx, decode/no-data): expected to
    ///   resolve on its own and must never count, or an offline window
    ///   (observer debounce fires every few seconds) would abandon the
    ///   whole queue well within `OfflineQueue.maxAttempts`.
    nonisolated private static func isDeterministicRejection(_ error: Error) -> Bool {
        guard let networkError = error as? NetworkError,
              case let .serverError(statusCode, _) = networkError else { return false }
        guard (400..<500).contains(statusCode) else { return false }
        return ![401, 403, 408, 429].contains(statusCode)
    }

    /// Runs `syncType` over `mappings` with bounded concurrency.
    /// Each in-flight task occupies one slot; the dispatcher waits for one
    /// to finish before starting another. Failures in any single type are
    /// recorded against the progress object and never bubble out — the
    /// remaining types keep flowing.
    private func runTypeSyncs(_ mappings: [HealthKitTypeMap.Mapping]) async {
        await withTaskGroup(of: Void.self) { group in
            var iterator = mappings.makeIterator()
            var inFlight = 0

            // Prime the group with up to maxConcurrentTypes initial tasks.
            while inFlight < maxConcurrentTypes, let next = iterator.next() {
                group.addTask { [self] in await self.runSingleType(next) }
                inFlight += 1
            }

            // For each completion, spawn the next mapping so we always have
            // at most `maxConcurrentTypes` running concurrently.
            while await group.next() != nil {
                if let next = iterator.next() {
                    group.addTask { [self] in await self.runSingleType(next) }
                }
            }
        }
    }

    private func runSingleType(_ mapping: HealthKitTypeMap.Mapping) async {
        let recordType = mapping.recordType
        await MainActor.run { progress.markSyncing(recordType) }
        do {
            let count = try await syncType(mapping)
            if count > 0 {
                await MainActor.run { progress.markSynced(recordType, count: count, at: Date()) }
            } else {
                await MainActor.run { progress.markSkipped(recordType) }
            }
        } catch {
            await MainActor.run { progress.markFailed(recordType, error: error.localizedDescription) }
            // Continue — don't halt other types
        }
    }

    /// One upload-sized chunk of samples cut from a single HealthKit page.
    /// `pageIndex` groups every chunk cut from the same page (a page can
    /// produce many chunks — `pageSize` / `batchSize` = up to 10 here) so
    /// the consumer can tell whether ALL of a page's chunks were acked
    /// before trusting that page's anchor; `pageAnchor` is what gets
    /// persisted once that's confirmed.
    private struct PagedBatch: Sendable {
        let samples: [HealthKitSample]
        /// Anchor returned by `querySamples` for the page this batch was
        /// cut from. Identical for every chunk from the same page; `nil`
        /// only if HealthKit didn't give us one (shouldn't happen in
        /// practice but we tolerate it).
        let pageAnchor: Data?
        /// Monotonically increasing per HealthKit page fetched (not per
        /// batch) — identical across every chunk cut from the same page.
        let pageIndex: Int
    }

    /// Returns the total number of records synced across all pages.
    ///
    /// Pipelining strategy:
    /// - Producer pages HealthKit (`querySamples(..., limit: pageSize)`),
    ///   chunks each page into upload-sized batches, and yields them into
    ///   an unbounded AsyncStream. The stream is unbounded so back-pressure
    ///   from slow uploads never silently drops batches; memory is bounded
    ///   organically because the producer can only run as fast as
    ///   `querySamples` returns.
    /// - Consumer drains the stream and uploads batches sequentially.
    /// - Anchor safety: a page's anchor is a single resume point covering
    ///   every sample HealthKit returned for that page — including the
    ///   ones cut into batches we never got to. So it can only be trusted
    ///   once EVERY batch cut from that page is acked (uploaded or
    ///   durably enqueued); the consumer tracks this per page (via
    ///   `pageIndex`) and only updates the persisted anchor at a page
    ///   boundary once that holds. If any batch anywhere in the run is
    ///   acked by NEITHER path (both upload and enqueue failed), the
    ///   anchor freezes at the last page that fully qualified before that
    ///   point — never the page the loss happened in, and never any page
    ///   after it, because the anchor is monotonic and a later "clean"
    ///   page can't undo an earlier hole.
    @discardableResult
    private func syncType(_ mapping: HealthKitTypeMap.Mapping) async throws -> Int {
        let recordType = mapping.recordType
        let hkType = mapping.hkType
        let batchSize = self.batchSize
        let pageSize = self.pageSize

        // Unbounded AsyncStream. `.bufferingNewest(N)` would silently drop
        // OLDEST batches under upload back-pressure (data loss). Memory is
        // bounded by the rate at which `querySamples` can produce pages —
        // the producer can't outrun HealthKit by very much.
        let (stream, continuation) = AsyncStream<PagedBatch>.makeStream(
            bufferingPolicy: .unbounded
        )

        // Per-type running counters. Actor-isolated because the producer
        // and consumer Tasks below may both touch them on different
        // executors. Cheap: one alloc per type, no contention in practice.
        actor TypeCounters {
            var total = 0
            var uploaded = 0
            func addTotal(_ n: Int) { total += n }
            func addUploaded(_ n: Int) { uploaded += n }
        }
        let counters = TypeCounters()

        // Producer: page through HealthKit, yielding upload-sized batches
        // each tagged with the page-level anchor.
        //
        // Invariant: `continuation.finish()` is called EXACTLY ONCE on
        // every exit path — normal completion, cancellation, OR throw.
        // Without the catch+finish, a HealthKit read error on the first
        // call would leave the consumer's `for await` hanging forever
        // because the stream would never terminate.
        let producerTask = Task { [healthKitProvider, anchorStore] () -> Void in
            do {
                var currentAnchor = try anchorStore.anchor(forRecordType: recordType)
                var pageIndex = 0
                while !Task.isCancelled {
                    let page = try await healthKitProvider.querySamples(
                        type: hkType,
                        anchor: currentAnchor,
                        limit: pageSize
                    )

                    if !page.samples.isEmpty {
                        await counters.addTotal(page.samples.count)
                        let pageTotal = await counters.total
                        await MainActor.run { self.progress.setTotalSamples(recordType, total: pageTotal) }

                        // Chunk the page into upload-sized batches, each
                        // tagged with this page's anchor and index.
                        let samples = page.samples
                        var idx = 0
                        while idx < samples.count {
                            let end = min(idx + batchSize, samples.count)
                            continuation.yield(PagedBatch(
                                samples: Array(samples[idx..<end]),
                                pageAnchor: page.newAnchor,
                                pageIndex: pageIndex
                            ))
                            idx = end
                        }
                    }

                    if let newAnchor = page.newAnchor {
                        currentAnchor = newAnchor
                    }

                    // HealthKit returns < limit when the result set is
                    // exhausted; that's our terminator.
                    if page.samples.count < pageSize {
                        break
                    }
                    pageIndex += 1
                }
                continuation.finish()
            } catch {
                // Unconditional finish: any throw from the producer must
                // still terminate the stream or the consumer hangs.
                continuation.finish()
                throw error
            }
        }

        // Consumer: drain the stream, uploading each batch. Tracks
        // per-page whether every batch cut from it was acked, and only
        // commits a page's anchor as the persisted resume point once that
        // page is fully accounted for (see doc comment above).
        var uploadError: Error?
        // Anchor of the last page that was fully acked. Persisted at the
        // end — never an anchor for a page any of whose batches were lost.
        var lastAckedAnchor: Data?
        // Once ANY batch anywhere is acked by neither path (upload nor
        // durable enqueue), its samples are gone for good. From that point
        // on no further page — including the rest of the one it happened
        // in — may ever be committed, because the anchor is a single
        // monotonic resume point and a later page can't undo an earlier
        // loss.
        var enqueueFailed = false
        // State for the page currently being accumulated by the consumer.
        var pendingPageIndex: Int?
        var pendingPageAnchor: Data?
        var pendingPageFullyAcked = true

        // Commits `pendingPageAnchor` as the new resume point iff every
        // batch seen so far from that page was acked AND no permanent loss
        // has occurred anywhere yet in this run.
        func commitPendingPage() {
            guard !enqueueFailed, pendingPageFullyAcked, let anchor = pendingPageAnchor else { return }
            lastAckedAnchor = anchor
        }

        // `pendingPageIndex != batch.pageIndex` is only a valid "new page
        // started" signal because the stream is strictly FIFO with a
        // SINGLE producer and SINGLE consumer: the producer finishes
        // yielding every chunk of page N (in order) before it ever fetches
        // (and starts yielding chunks for) page N+1, and nothing else
        // writes to the stream. If either assumption changed (e.g.
        // multiple producers, or out-of-order delivery), batches from
        // different pages could interleave and this simple "did the index
        // change" check would no longer reliably mark a page boundary.
        func beginPageIfNeeded(_ batch: PagedBatch) {
            guard pendingPageIndex != batch.pageIndex else { return }
            commitPendingPage()
            pendingPageIndex = batch.pageIndex
            pendingPageAnchor = batch.pageAnchor
            pendingPageFullyAcked = true
        }

        for await pagedBatch in stream {
            beginPageIfNeeded(pagedBatch)

            let records = pagedBatch.samples.map { sample in
                CreateHealthRecord(
                    source: "healthkit",
                    recordType: sample.recordType,
                    value: sample.value,
                    unit: sample.unit,
                    startTime: sample.startTime,
                    endTime: sample.endTime,
                    metadata: nil,
                    sourceId: sample.sourceId
                )
            }
            let insert = HealthKitBulkInsert(records: records)
            do {
                try await networkClient.requestNoContent(
                    method: "POST",
                    path: Endpoints.healthKitSync,
                    body: insert
                )
                await counters.addUploaded(pagedBatch.samples.count)
                let running = await counters.uploaded
                await MainActor.run {
                    self.progress.updateUploadProgress(recordType, uploaded: running)
                }
                // Uploaded — this batch is acked; page status unchanged.
            } catch {
                // The wire upload failed. Two things must happen before we
                // stop:
                //  1. Enqueue THIS batch for retry so it isn't lost.
                //  2. Drain the REST of the stream into the offline queue
                //     too — those samples have already been read out of
                //     HealthKit and would be skipped on the next run
                //     because the anchor will have moved past them.
                do {
                    try offlineQueue.enqueue(insert)
                    // Durably queued — acked; page status unchanged.
                } catch {
                    enqueueFailed = true
                    pendingPageFullyAcked = false
                    engineLogger.error("offline queue enqueue failed: type=\(recordType, privacy: .public) errorType=\(String(describing: type(of: error)), privacy: .public)")
                }
                logUploadFailure(error, context: "type=\(recordType)")
                uploadError = error

                // Stop the producer from fetching MORE pages, but keep
                // consuming what's already buffered so we can stash it.
                producerTask.cancel()

                for await leftover in stream {
                    beginPageIfNeeded(leftover)

                    let leftoverRecords = leftover.samples.map { sample in
                        CreateHealthRecord(
                            source: "healthkit",
                            recordType: sample.recordType,
                            value: sample.value,
                            unit: sample.unit,
                            startTime: sample.startTime,
                            endTime: sample.endTime,
                            metadata: nil,
                            sourceId: sample.sourceId
                        )
                    }
                    let leftoverInsert = HealthKitBulkInsert(records: leftoverRecords)
                    do {
                        try offlineQueue.enqueue(leftoverInsert)
                        // Durably queued — acked; page status unchanged.
                    } catch {
                        enqueueFailed = true
                        pendingPageFullyAcked = false
                        engineLogger.error("offline queue enqueue failed: type=\(recordType, privacy: .public) errorType=\(String(describing: type(of: error)), privacy: .public)")
                    }
                }
                break
            }
        }

        // Finalize whichever page was last being accumulated — reached via
        // normal stream completion or via `break` after the failure-drain
        // loop above.
        commitPendingPage()

        // Wait for producer to finish (so we can surface any read error).
        do {
            try await producerTask.value
        } catch {
            // Producer threw — likely a HealthKit read failure. Surface it
            // only if we don't already have an upload error to report.
            if uploadError == nil {
                uploadError = error
            }
        }

        // Persist only the last fully-acknowledged page's anchor. If
        // nothing was ever fully acked, leave the prior anchor alone so
        // the next sync retries from the same point.
        if let lastAckedAnchor {
            try? anchorStore.saveAnchor(lastAckedAnchor, forRecordType: recordType)
        }

        if let uploadError {
            throw uploadError
        }
        return await counters.total
    }

    /// Categorize the failure mode and log it (no PHI). Helps triage device
    /// logs when a sync stalls in the field — we want to know "401 vs 5xx
    /// vs network" without sniffing the wire.
    ///
    /// `.decodingFailed` / `.noData` are defense-in-depth: the upload path
    /// uses `requestNoContent` which never decodes a body, so they should
    /// be unreachable from here, but exhaustive switch coverage keeps
    /// `NetworkError` additions from silently slipping past the logger.
    nonisolated private func logUploadFailure(_ error: Error, context: String) {
        let mode: String
        if let net = error as? NetworkError {
            switch net {
            case .unauthorized:
                mode = "auth"
            case .serverError(let code, _) where code == 401 || code == 403:
                mode = "auth"
            case .serverError(let code, _) where (400..<500).contains(code):
                mode = "client-4xx-\(code)"
            case .serverError(let code, _):
                mode = "server-\(code)"
            case .decodingFailed:
                mode = "decode"
            case .noData:
                mode = "no-data"
            }
        } else {
            let nserr = error as NSError
            if nserr.domain == NSURLErrorDomain {
                mode = "network-\(nserr.code)"
            } else {
                mode = "other"
            }
        }
        engineLogger.error("HealthKit batch upload failed: mode=\(mode, privacy: .public) context=\(context, privacy: .public)")
    }

    private func syncClinicalRecords(_ provider: ClinicalRecordProviderProtocol) async throws {
        let anchor = try anchorStore.anchor(forRecordType: "clinical_lab_result")
        let result = try await provider.queryLabResults(anchor: anchor)

        guard !result.results.isEmpty else {
            if let newAnchor = result.newAnchor {
                try anchorStore.saveAnchor(newAnchor, forRecordType: "clinical_lab_result")
            }
            return
        }

        let dateFormatter = DateFormatter()
        dateFormatter.dateFormat = "yyyy-MM-dd"
        dateFormatter.locale = Locale(identifier: "en_US_POSIX")
        dateFormatter.timeZone = TimeZone(identifier: "UTC")

        let batches = stride(from: 0, to: result.results.count, by: batchSize).map {
            Array(result.results[$0..<min($0 + batchSize, result.results.count)])
        }

        for batch in batches {
            let records = batch.map { lab in
                CreateLabResultRecord(
                    panelDate: dateFormatter.string(from: lab.panelDate),
                    labName: lab.labName,
                    marker: lab.marker,
                    value: lab.value,
                    unit: lab.unit,
                    referenceLow: lab.referenceLow,
                    referenceHigh: lab.referenceHigh,
                    source: "apple_health_records",
                    sourceId: lab.sourceId
                )
            }
            let body = BulkCreateLabResults(records: records)
            let _: [LabResultResponse] = try await networkClient.request(
                method: "POST",
                path: Endpoints.labsBulk,
                body: body
            )
        }

        if let newAnchor = result.newAnchor {
            try anchorStore.saveAnchor(newAnchor, forRecordType: "clinical_lab_result")
        }
    }

    #if swift(>=6.3)
    @available(iOS 26.0, *)
    private func syncMedicationDoses(_ provider: MedicationSyncProviderProtocol) async throws {
        let anchorKey = "medication_dose_event"
        let anchor = try anchorStore.anchor(forRecordType: anchorKey)
        let result = try await provider.queryDoseEvents(anchor: anchor)

        guard !result.records.isEmpty else {
            if let newAnchor = result.newAnchor {
                try anchorStore.saveAnchor(newAnchor, forRecordType: anchorKey)
            }
            return
        }

        let formatter = ISO8601DateFormatter()

        for batch in stride(from: 0, to: result.records.count, by: batchSize).map({
            Array(result.records[$0..<min($0 + batchSize, result.records.count)])
        }) {
            let interventions = batch.map { record in
                CreateIntervention(
                    substance: record.substance,
                    dose: record.dose,
                    unit: record.unit,
                    route: record.route,
                    administeredAt: formatter.string(from: record.administeredAt),
                    fasted: false,
                    notes: "Synced from Apple Health"
                )
            }

            for intervention in interventions {
                let _: InterventionResponse = try await networkClient.request(
                    method: "POST",
                    path: Endpoints.interventions,
                    body: intervention
                )
            }
        }

        if let newAnchor = result.newAnchor {
            try anchorStore.saveAnchor(newAnchor, forRecordType: anchorKey)
        }
    }
    #endif

    private func processWriteBack() async throws {
        let items: [HealthKitWriteQueueItem] = try await networkClient.request(
            method: "GET",
            path: Endpoints.healthKitWriteQueue,
            body: nil as String?
        )

        guard !items.isEmpty else { return }

        var confirmedIDs: [String] = []

        for item in items {
            guard let mapping = HealthKitTypeMap.mapping(forRecordType: item.hkType) else {
                continue
            }

            do {
                try await healthKitProvider.writeSample(
                    type: mapping.hkType,
                    value: item.value,
                    unit: mapping.unit,
                    start: item.scheduledAt,
                    end: item.scheduledAt
                )
                confirmedIDs.append(item.id)
            } catch {
                // Skip failed writes — server will retry
            }
        }

        if !confirmedIDs.isEmpty {
            try await networkClient.requestNoContent(
                method: "POST",
                path: Endpoints.healthKitConfirm,
                body: HealthKitConfirm(ids: confirmedIDs)
            )
        }
    }
}

/// Actor-isolated holder for the background task identifier. Guards against
/// the expiration handler (which fires on the main thread) racing with the
/// normal post-sync cleanup — both call `endIfNeeded`, and the second call
/// is a no-op.
private actor TaskHandle {
    private var id: Int = invalidBackgroundTask
    private var ended = false

    func setId(_ newId: Int) {
        id = newId
    }

    func endIfNeeded(host: BackgroundTaskHost) async {
        guard !ended, id != invalidBackgroundTask else { return }
        ended = true
        let taskId = id
        await MainActor.run {
            host.endBackgroundTask(taskId)
        }
    }
}
