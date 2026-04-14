// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit
import Observation

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
    private let batchSize = 100

    private var _isSyncing = false
    private var _lastSyncDate: Date?
    private var _lastError: String?

    // Expose to callers that need to await actor-isolated state
    var isSyncing: Bool { _isSyncing }
    var lastSyncDate: Date? { _lastSyncDate }
    var lastError: String? { _lastError }

    init(
        networkClient: NetworkClientProtocol,
        healthKitProvider: HealthKitProviderProtocol,
        clinicalRecordProvider: ClinicalRecordProviderProtocol? = nil,
        medicationSyncProvider: (any Sendable)? = nil,
        offlineQueue: OfflineQueueProtocol,
        anchorStore: AnchorStore
    ) {
        self.networkClient = networkClient
        self.healthKitProvider = healthKitProvider
        self.clinicalRecordProvider = clinicalRecordProvider
        self.medicationSyncProvider = medicationSyncProvider
        self.offlineQueue = offlineQueue
        self.anchorStore = anchorStore
    }

    func sync() async {
        guard !_isSyncing else { return }
        _isSyncing = true
        _lastError = nil

        defer { _isSyncing = false }

        do {
            // 1. Drain offline queue first
            try await drainOfflineQueue()

            // 2. Query HK for each type and upload
            for mapping in HealthKitTypeMap.mappings {
                try await syncType(mapping)
            }

            // 3. Sync clinical records (lab results from Apple Health Records) if enabled
            if let clinicalProvider = clinicalRecordProvider,
               ClinicalRecordSettings.isSyncEnabled {
                do {
                    try await syncClinicalRecords(clinicalProvider)
                } catch {
                    // Don't fail the entire sync if clinical records fail
                    _lastError = "Clinical records sync failed: \(error.localizedDescription)"
                }
            }

            // 4. Sync medication dose events (iOS 26+)
            #if swift(>=6.1)
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

            // 5. Process write-back queue
            try await processWriteBack()

            _lastSyncDate = Date()
        } catch {
            _lastError = error.localizedDescription
        }
    }

    private func drainOfflineQueue() async throws {
        let pending = try offlineQueue.dequeuePending()
        for entry in pending {
            do {
                let _: [HealthRecordResponse] = try await networkClient.request(
                    method: "POST",
                    path: Endpoints.healthKitSync,
                    body: entry.insert
                )
                try offlineQueue.markComplete(id: entry.id)
            } catch {
                // Leave in queue for next sync
                throw error
            }
        }
    }

    private func syncType(_ mapping: HealthKitTypeMap.Mapping) async throws {
        let anchor = try anchorStore.anchor(forRecordType: mapping.recordType)
        let result = try await healthKitProvider.querySamples(
            type: mapping.hkType,
            anchor: anchor
        )

        guard !result.samples.isEmpty else {
            if let newAnchor = result.newAnchor {
                try anchorStore.saveAnchor(newAnchor, forRecordType: mapping.recordType)
            }
            return
        }

        // Batch upload
        let batches = stride(from: 0, to: result.samples.count, by: batchSize).map {
            Array(result.samples[$0..<min($0 + batchSize, result.samples.count)])
        }

        for batch in batches {
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
                let _: [HealthRecordResponse] = try await networkClient.request(
                    method: "POST",
                    path: Endpoints.healthKitSync,
                    body: insert
                )
            } catch {
                // Queue for offline retry
                try offlineQueue.enqueue(insert)
                throw error
            }
        }

        if let newAnchor = result.newAnchor {
            try anchorStore.saveAnchor(newAnchor, forRecordType: mapping.recordType)
        }
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

    #if swift(>=6.1)
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
