// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "healthkit")

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

/// Read-permission status for a single HealthKit type.
/// Mirrors `HKAuthorizationStatus` but is exposed at the protocol level so
/// tests can stub it without faking a real `HKHealthStore`.
enum HealthKitReadAuthorizationStatus: Sendable {
    case notDetermined
    case sharingDenied
    case sharingAuthorized
}

protocol HealthKitProviderProtocol: Sendable {
    func requestAuthorization() async throws
    func isAuthorized() -> Bool

    /// Returns the current authorization status for `type`. iOS only reports
    /// share (write) status accurately; for read status we treat any
    /// non-`.notDetermined` value as authorized — this is good enough for
    /// the diagnostic logging in `AppDependencies.bootstrapAutoSync()`.
    func authorizationStatus(for type: HKObjectType) -> HealthKitReadAuthorizationStatus

    /// Read up to `limit` samples newer than `anchor`. Pass a finite limit
    /// (e.g. 5000) when backfilling large types so the consumer can start
    /// uploading without waiting for the full result set to materialize.
    /// Callers loop, feeding the returned `newAnchor` back in until the
    /// result is empty.
    func querySamples(
        type: HKSampleType,
        anchor: Data?,
        limit: Int
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

    /// Disable all background-delivery registrations set up by
    /// `enableBackgroundDelivery()`. Call on logout so iOS doesn't keep
    /// waking the app for a user that's no longer signed in.
    func disableAllBackgroundDelivery() async throws
}

final class HealthKitProvider: HealthKitProviderProtocol, @unchecked Sendable {
    private let store = HKHealthStore()

    /// Record types that use `.immediate` background-delivery frequency.
    /// Extracted as a pure lookup so the policy can be unit-tested without
    /// a real HKHealthStore — see `HealthKitProviderTests`.
    ///
    /// Rationale: `.immediate` keeps latency low for real-time metrics
    /// (workouts, blood-oxygen spikes) where users expect the OwnPulse
    /// server to reflect Apple Health within minutes. Everything else is
    /// `.hourly` to stay gentle on iOS's power budget — and iOS throttles
    /// `.immediate` itself under thermal/battery pressure, so this is a
    /// hint, not a contract.
    static let immediateDeliveryRecordTypes: Set<String> = ["heart_rate", "blood_oxygen"]

    /// Pure helper: returns the background-delivery frequency for a given
    /// record type. Tests pin this to guard against new mappings silently
    /// inheriting the wrong policy.
    static func backgroundDeliveryFrequency(for recordType: String) -> HKUpdateFrequency {
        immediateDeliveryRecordTypes.contains(recordType) ? .immediate : .hourly
    }

    func requestAuthorization() async throws {
        // HealthKit's `requestAuthorization` raises an `NSException` (not an
        // `NSError`) if any type in `toShare` is disallowed — e.g. Apple
        // restricts writing for certain derived/event types, or the current
        // iOS build disallows a type that was writable in a prior SDK.
        // Swift can't catch Objective-C exceptions, so the raw call crashes
        // the process with SIGABRT. Wrap in our ObjC bridge so the exception
        // becomes a Swift-catchable error and the caller gets a proper
        // "not connected" state instead of a crash.
        //
        // If this path triggers in production, the offending type(s) can be
        // found by running `probeAuthorizationForWriteTypes` which submits
        // each write type individually.
        //
        // Swift imports `+tryBlock:error:` as a `throws` function (the
        // classic NSError-out-pointer pattern), so we use try/catch here,
        // not a Bool return.
        let store = self.store
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            do {
                try ObjCExceptionCatcher.`try` {
                    store.requestAuthorization(
                        toShare: HealthKitTypeMap.allWriteTypes,
                        read: HealthKitTypeMap.allReadTypes
                    ) { _, error in
                        if let error {
                            continuation.resume(throwing: error)
                        } else {
                            continuation.resume()
                        }
                    }
                }
                // Success path: completion handler will resume the continuation.
            } catch {
                // NSException path: completion never registered.
                continuation.resume(throwing: error)
            }
        }
    }

    /// Diagnostic helper: requests authorization for each write type in
    /// isolation and returns the ones whose HealthKit call raised an
    /// `NSException`. Use from a debug UI or a test to narrow down which
    /// specific types are disallowed on the current OS without crashing.
    /// This does NOT mutate authorization state for types that succeed —
    /// it only triggers the up-front type validation.
    #if DEBUG
    func probeAuthorizationForWriteTypes() -> [String] {
        var failing: [String] = []
        let store = self.store
        for mapping in HealthKitTypeMap.mappings where mapping.writable {
            do {
                try ObjCExceptionCatcher.`try` {
                    store.requestAuthorization(
                        toShare: [mapping.hkType],
                        read: []
                    ) { _, _ in }
                }
            } catch {
                failing.append(mapping.recordType)
            }
        }
        return failing
    }
    #endif

    func isAuthorized() -> Bool {
        HKHealthStore.isHealthDataAvailable()
    }

    func authorizationStatus(for type: HKObjectType) -> HealthKitReadAuthorizationStatus {
        switch store.authorizationStatus(for: type) {
        case .notDetermined:
            return .notDetermined
        case .sharingDenied:
            return .sharingDenied
        case .sharingAuthorized:
            return .sharingAuthorized
        @unknown default:
            return .notDetermined
        }
    }

    func querySamples(
        type: HKSampleType,
        anchor: Data?,
        limit: Int
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
            // Cap each round trip at `limit`. The caller drives a paging
            // loop, so for a 500K-sample type we yield 5K-sample pages
            // instead of materializing the whole result up front.
            let query = HKAnchoredObjectQuery(
                type: type,
                predicate: nil,
                anchor: hkAnchor,
                limit: limit
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
                    // knows the delivery was handled. On error, log without
                    // sample IDs (no PHI) and still invoke completionHandler
                    // so HealthKit doesn't think we've hung. We skip the
                    // yield so the coordinator doesn't sync on noise.
                    if let error {
                        logger.error("HKObserverQuery delivery error: \(error.localizedDescription, privacy: .public)")
                    } else {
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
        for mapping in HealthKitTypeMap.mappings {
            let frequency = Self.backgroundDeliveryFrequency(for: mapping.recordType)
            try await store.enableBackgroundDelivery(for: mapping.hkType, frequency: frequency)
        }
    }

    func disableAllBackgroundDelivery() async throws {
        // HKHealthStore exposes `disableAllBackgroundDelivery(completion:)`
        // which is the correct call on logout — it blanket-tears-down every
        // enable registration this app made, including ones from older
        // sessions whose types we may no longer register for.
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            store.disableAllBackgroundDelivery { success, error in
                if let error {
                    continuation.resume(throwing: error)
                } else if !success {
                    // HealthKit returned (false, nil) — undocumented but
                    // historically means "nothing to disable". Treat as OK.
                    continuation.resume()
                } else {
                    continuation.resume()
                }
            }
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
