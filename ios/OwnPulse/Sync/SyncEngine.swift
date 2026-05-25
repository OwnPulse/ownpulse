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
            }
        }
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

    /// Returns the total number of records synced across all pages.
    ///
    /// Strategy: page through `HealthKitProvider.querySamples(..., limit:)`,
    /// feeding each page into a bounded `AsyncStream` of upload batches.
    /// A single consumer drains the stream and uploads batches sequentially
    /// (one in-flight upload per type, keeping ordering simple and letting
    /// the outer task group provide cross-type concurrency).
    /// Producer and consumer overlap — the next HealthKit page is fetched
    /// while the previous batches are uploading.
    @discardableResult
    private func syncType(_ mapping: HealthKitTypeMap.Mapping) async throws -> Int {
        let recordType = mapping.recordType
        let hkType = mapping.hkType
        let batchSize = self.batchSize
        let pageSize = self.pageSize

        // Bounded AsyncStream — buffer 2 batches keeps the producer one
        // step ahead of the consumer without piling samples in memory.
        let (stream, continuation) = AsyncStream<[HealthKitSample]>.makeStream(
            bufferingPolicy: .bufferingNewest(2)
        )

        // Per-type running counters. Held in `nonisolated(unsafe)` boxes
        // because the producer/consumer Tasks below need to mutate them;
        // they never run concurrently for the same type — the producer
        // writes `total`/`anchor`, the consumer writes `uploaded`.
        actor TypeCounters {
            var total = 0
            var uploaded = 0
            var finalAnchor: Data?
            func addTotal(_ n: Int) { total += n }
            func addUploaded(_ n: Int) { uploaded += n }
            func setAnchor(_ data: Data?) { if data != nil { finalAnchor = data } }
        }
        let counters = TypeCounters()

        // Producer: page through HealthKit, yielding upload-sized batches.
        let producerTask = Task { [healthKitProvider, anchorStore] in
            var currentAnchor = try anchorStore.anchor(forRecordType: recordType)
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

                    // Chunk the page into upload-sized batches.
                    let samples = page.samples
                    var idx = 0
                    while idx < samples.count {
                        let end = min(idx + batchSize, samples.count)
                        continuation.yield(Array(samples[idx..<end]))
                        idx = end
                    }
                }

                // Advance the anchor — both for the next page in this loop
                // AND to persist progress so a crash doesn't lose ground.
                if let newAnchor = page.newAnchor {
                    await counters.setAnchor(newAnchor)
                    currentAnchor = newAnchor
                }

                // HealthKit returns < limit when the result set is exhausted.
                // An empty page after an empty anchor advance also means we're done.
                if page.samples.count < pageSize {
                    break
                }
            }
            continuation.finish()
        }

        // Consumer: drain the stream, uploading each batch.
        var uploadError: Error?
        for await batch in stream {
            let records = batch.map { sample in
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
                await counters.addUploaded(batch.count)
                let running = await counters.uploaded
                await MainActor.run {
                    self.progress.updateUploadProgress(recordType, uploaded: running)
                }
            } catch {
                // Queue for offline retry, log the failure mode, and stop
                // this type's pipeline. The outer loop continues with other
                // types so a single broken endpoint doesn't kill the run.
                try? offlineQueue.enqueue(insert)
                logUploadFailure(error, context: "type=\(recordType)")
                uploadError = error
                producerTask.cancel()
                break
            }
        }

        // Wait for producer to finish (so we can read its anchor + surface errors).
        do {
            try await producerTask.value
        } catch {
            // Producer threw — likely a HealthKit read failure. Surface it
            // if we don't already have an upload error to report.
            if uploadError == nil {
                uploadError = error
            }
        }

        // Persist the latest anchor we saw, even on partial-failure paths —
        // the rows we already uploaded shouldn't be re-fetched next time.
        if let finalAnchor = await counters.finalAnchor {
            try? anchorStore.saveAnchor(finalAnchor, forRecordType: recordType)
        }

        if let uploadError {
            throw uploadError
        }
        return await counters.total
    }

    /// Categorize the failure mode and log it (no PHI). Helps triage device
    /// logs when a sync stalls in the field — we want to know "401 vs 5xx
    /// vs network" without sniffing the wire.
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
