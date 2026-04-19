// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit

struct HealthKitSample: Sendable {
    let recordType: String
    let value: Double
    let unit: String
    let startTime: Date
    let endTime: Date
    let sourceId: String
}

struct AnchoredQueryResult: Sendable {
    let samples: [HealthKitSample]
    let newAnchor: Data?
    let deletedObjectIDs: [String]
}

protocol HealthKitProviderProtocol: Sendable {
    func requestAuthorization() async throws
    func isAuthorized() -> Bool
    func querySamples(
        type: HKSampleType,
        anchor: Data?
    ) async throws -> AnchoredQueryResult
    func writeSample(
        type: HKSampleType,
        value: Double,
        unit: HKUnit,
        start: Date,
        end: Date
    ) async throws

    /// Emits a `Void` each time HealthKit notifies the app of new samples for
    /// any of the configured read types. The stream stays open until the
    /// returned task handle is cancelled via `.finish()`/termination.
    ///
    /// Callers should debounce this stream — HealthKit fires it eagerly during
    /// bulk writes (e.g. during a workout) and we don't want to kick off a
    /// network sync for every individual heartbeat sample.
    func observeSampleUpdates() -> AsyncStream<Void>

    /// After authorization, enable iOS to wake the app in the background when
    /// new samples are written for the given types. Safe to call more than
    /// once — HealthKit coalesces repeated registrations.
    func enableBackgroundDelivery() async throws
}

final class HealthKitProvider: HealthKitProviderProtocol, @unchecked Sendable {
    private let store = HKHealthStore()

    func requestAuthorization() async throws {
        try await store.requestAuthorization(
            toShare: HealthKitTypeMap.allWriteTypes,
            read: HealthKitTypeMap.allReadTypes
        )
    }

    func isAuthorized() -> Bool {
        HKHealthStore.isHealthDataAvailable()
    }

    func querySamples(
        type: HKSampleType,
        anchor: Data?
    ) async throws -> AnchoredQueryResult {
        guard let mapping = HealthKitTypeMap.mapping(forHKType: type) else {
            return AnchoredQueryResult(samples: [], newAnchor: nil, deletedObjectIDs: [])
        }

        let hkAnchor: HKQueryAnchor?
        if let anchorData = anchor {
            hkAnchor = try NSKeyedUnarchiver.unarchivedObject(
                ofClass: HKQueryAnchor.self,
                from: anchorData
            )
        } else {
            hkAnchor = nil
        }

        return try await withCheckedThrowingContinuation { continuation in
            let query = HKAnchoredObjectQuery(
                type: type,
                predicate: nil,
                anchor: hkAnchor,
                limit: HKObjectQueryNoLimit
            ) { _, added, deleted, newAnchor, error in
                if let error {
                    continuation.resume(throwing: error)
                    return
                }

                let samples = (added ?? []).compactMap { sample -> HealthKitSample? in
                    if let quantitySample = sample as? HKQuantitySample {
                        return HealthKitSample(
                            recordType: mapping.recordType,
                            value: quantitySample.quantity.doubleValue(for: mapping.unit),
                            unit: mapping.unitString,
                            startTime: quantitySample.startDate,
                            endTime: quantitySample.endDate,
                            sourceId: sample.uuid.uuidString
                        )
                    } else if let categorySample = sample as? HKCategorySample {
                        return HealthKitSample(
                            recordType: mapping.recordType,
                            value: Double(categorySample.value),
                            unit: mapping.unitString,
                            startTime: categorySample.startDate,
                            endTime: categorySample.endDate,
                            sourceId: sample.uuid.uuidString
                        )
                    }
                    return nil
                }

                var anchorData: Data?
                if let newAnchor {
                    anchorData = try? NSKeyedArchiver.archivedData(
                        withRootObject: newAnchor,
                        requiringSecureCoding: true
                    )
                }

                let deletedIDs = (deleted ?? []).map { $0.uuid.uuidString }

                continuation.resume(returning: AnchoredQueryResult(
                    samples: samples,
                    newAnchor: anchorData,
                    deletedObjectIDs: deletedIDs
                ))
            }

            store.execute(query)
        }
    }

    func writeSample(
        type: HKSampleType,
        value: Double,
        unit: HKUnit,
        start: Date,
        end: Date
    ) async throws {
        guard let quantityType = type as? HKQuantityType else { return }
        let quantity = HKQuantity(unit: unit, doubleValue: value)
        let sample = HKQuantitySample(
            type: quantityType,
            quantity: quantity,
            start: start,
            end: end
        )
        try await store.save(sample)
    }

    func observeSampleUpdates() -> AsyncStream<Void> {
        AsyncStream { continuation in
            // Retain the running queries so we can stop them when the stream
            // terminates. HealthKit keeps observer queries alive between app
            // launches via background delivery, but we stop ours explicitly
            // on logout/stream cancellation to avoid duplicate notifications.
            let sampleTypes = HealthKitTypeMap.mappings.compactMap { $0.hkType as? HKSampleType }
            let queries = QueryBag()

            for sampleType in sampleTypes {
                let query = HKObserverQuery(sampleType: sampleType, predicate: nil) { _, completionHandler, error in
                    // HealthKit expects us to call `completionHandler` so it
                    // knows the delivery was handled. Even on error, yield to
                    // let the sync engine decide whether to retry.
                    if error == nil {
                        continuation.yield()
                    }
                    completionHandler()
                }
                store.execute(query)
                queries.append(query)
            }

            continuation.onTermination = { [queries, store] _ in
                for query in queries.snapshot() {
                    store.stop(query)
                }
            }
        }
    }

    func enableBackgroundDelivery() async throws {
        // Frequency choice: we use `.hourly` for most quantity types to avoid
        // hammering iOS's power budget. `heartRate` and `oxygenSaturation` use
        // `.immediate` so real-time events (workouts, blood-oxygen spikes)
        // sync promptly. This is a deliberate trade-off — longer-latency
        // types don't need sub-hour freshness, and iOS throttles .immediate
        // under thermal/battery pressure anyway.
        let immediateRecordTypes: Set<String> = ["heart_rate", "blood_oxygen"]

        for mapping in HealthKitTypeMap.mappings {
            let frequency: HKUpdateFrequency = immediateRecordTypes.contains(mapping.recordType)
                ? .immediate
                : .hourly
            try await store.enableBackgroundDelivery(for: mapping.hkType, frequency: frequency)
        }
    }
}

/// Thread-safe container for HKObserverQuery instances held by the observer
/// stream. Exists only so the `onTermination` closure can stop queries
/// without capturing a mutable array.
private final class QueryBag: @unchecked Sendable {
    private let lock = NSLock()
    private var queries: [HKObserverQuery] = []

    func append(_ query: HKObserverQuery) {
        lock.lock(); defer { lock.unlock() }
        queries.append(query)
    }

    func snapshot() -> [HKObserverQuery] {
        lock.lock(); defer { lock.unlock() }
        return queries
    }
}
