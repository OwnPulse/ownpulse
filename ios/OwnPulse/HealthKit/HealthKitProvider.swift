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

/// Errors `HealthKitProvider.writeSample` can throw beyond whatever
/// `HKHealthStore.save` itself raises.
enum HealthKitWriteError: Error, Sendable, LocalizedError {
    /// `type` isn't an `HKQuantityType` — there's no quantity-sample write
    /// path for it (e.g. category types like sleep analysis).
    case unsupportedSampleType(String)

    var errorDescription: String? {
        switch self {
        case .unsupportedSampleType(let identifier):
            // Type identifier only — never sample values — so this is safe
            // to surface in logs and in the write-back failure sent to the
            // backend.
            return "Unsupported HealthKit sample type: \(identifier)"
        }
    }
}

/// Classifies a `writeSample` failure as **deterministic** (retrying won't
/// help — e.g. the sample is malformed, or the type can never be written) or
/// **transient** (a later retry might succeed — e.g. HealthKit is temporarily
/// unavailable, the device is locked, or authorization hasn't been decided
/// yet).
///
/// This distinction matters because reporting an item in `/healthkit/confirm`'s
/// `failures` **permanently retires it** server-side (same as a real
/// confirmation) — see backend PR healthkit-writeback-failure-reporting. If a
/// transient write failure were reported as a `failures` entry, the item
/// would never be retried even though the underlying condition (locked
/// device, HealthKit temporarily unavailable) was expected to clear on its
/// own. Transient failures are instead skipped silently: the item stays
/// pending and the write-queue's natural retry (next sync) picks it up
/// again. Only deterministic failures — where retrying is pointless — go
/// into `failures`.
enum WriteBackFailureClassifier {
    /// `true` if `error` should be reported to the backend as a permanent
    /// failure; `false` if it should be left pending for the next sync.
    /// Unrecognized errors default to `false` (transient) — the safer
    /// direction, since misclassifying a real permanent failure as
    /// transient only costs a wasted retry, while misclassifying a
    /// transient failure as permanent throws away data forever.
    static func isDeterministic(_ error: Error) -> Bool {
        // `HealthKitWriteError.unsupportedSampleType` — a category type can
        // never be written as a quantity sample, no matter how many times
        // we retry.
        if error is HealthKitWriteError {
            return true
        }

        guard let hkError = error as? HKError else {
            return false
        }

        switch hkError.code {
        case .errorInvalidArgument:
            // The sample itself is malformed (e.g. bad unit/value combo) —
            // retrying with the same data will fail the same way.
            return true
        case .errorAuthorizationDenied, .errorRequiredAuthorizationDenied:
            // The user has explicitly denied share authorization for this
            // type — this is the motivating head-of-line case (the pact
            // example failure string is literally
            // "HealthKit authorization denied for Body Mass"). Nothing about
            // retrying changes this outcome; it needs a user action
            // (re-granting in Settings), which re-enqueues a fresh item
            // rather than un-sticking this one.
            return true
        case .errorHealthDataUnavailable,
             .errorHealthDataRestricted,
             .errorAuthorizationNotDetermined,
             .errorDatabaseInaccessible:
            // Store temporarily unavailable, device locked (data
            // protection), or authorization mid-flow (the user hasn't been
            // asked yet, as opposed to having said no) — all expected to
            // clear on their own.
            return false
        default:
            return false
        }
    }
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
    /// Writes a quantity sample, tagged with `syncIdentifier` (HealthKit's
    /// `HKMetadataKeySyncIdentifier`/`HKMetadataKeySyncVersion` pair) so a
    /// re-write of the same write-queue item — e.g. if the confirm POST
    /// fails after the HealthKit write already succeeded, leaving the item
    /// pending for the next sync — replaces the existing sample in place
    /// instead of creating a duplicate. HealthKit has no other de-dup
    /// mechanism for writes.
    func writeSample(
        type: HKSampleType,
        value: Double,
        unit: HKUnit,
        start: Date,
        end: Date,
        syncIdentifier: String
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

    /// Read-side cycle-prevention filter — [ADR-0008](../../../docs/decisions/0008-healthkit-sync.md).
    ///
    /// OwnPulse writes records it creates to HealthKit under its own
    /// `HKSource` (the app's bundle ID). Without a filter, the very next
    /// anchored read would pick those records back up, producing a
    /// write → read → re-upload cycle and duplicate data. This predicate
    /// excludes samples whose source is this app, unconditionally, on
    /// every read. It is not configurable and must be applied to every
    /// `HKAnchoredObjectQuery` over a type OwnPulse is able to write.
    ///
    /// `HKSource.default()` reflects the current process's bundle ID, which
    /// is stable and available in the unit test host, so tests can and do
    /// construct the real predicate here. What tests can't do is exercise
    /// this against a live `HKHealthStore` read — there's no way to seed
    /// real HealthKit samples "from this app" vs. "from elsewhere" in a
    /// unit test, so coverage is limited to shape (an `NSCompoundPredicate`
    /// of type `.not` wrapping exactly one subpredicate) and to confirming,
    /// via `makeAnchoredQuery`, that the predicate actually reaches the
    /// query passed to HealthKit.
    static func makeReadPredicate() -> NSPredicate {
        NSCompoundPredicate(
            notPredicateWithSubpredicate: HKQuery.predicateForObjects(from: HKSource.default())
        )
    }

    /// Builds the `HKAnchoredObjectQuery` used by `querySamples`, always
    /// wired up with `makeReadPredicate()`. Extracted as a static function
    /// (rather than inlined in `querySamples`) so a test can construct one
    /// and assert its `.predicate` actually carries the cycle-prevention
    /// filter — without this seam, a regression that dropped the predicate
    /// argument would still pass every existing test.
    static func makeAnchoredQuery(
        type: HKSampleType,
        anchor: HKQueryAnchor?,
        limit: Int,
        resultsHandler: @escaping (HKAnchoredObjectQuery, [HKSample]?, [HKDeletedObject]?, HKQueryAnchor?, Error?) -> Void
    ) -> HKAnchoredObjectQuery {
        HKAnchoredObjectQuery(
            type: type,
            predicate: makeReadPredicate(),
            anchor: anchor,
            limit: limit,
            resultsHandler: resultsHandler
        )
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
            let query = Self.makeAnchoredQuery(
                type: type,
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
        end: Date,
        syncIdentifier: String
    ) async throws {
        // Category types (e.g. sleep_analysis) can't be represented as an
        // `HKQuantitySample`. Silently no-op-ing here used to leave the
        // caller believing the write succeeded — it would go on to confirm
        // the write-queue item to the backend even though nothing was ever
        // written to Apple Health. Throw so the caller can report it as a
        // failure instead.
        guard let quantityType = type as? HKQuantityType else {
            throw HealthKitWriteError.unsupportedSampleType(type.identifier)
        }
        let quantity = HKQuantity(unit: unit, doubleValue: value)
        // `HKMetadataKeySyncIdentifier`/`HKMetadataKeySyncVersion` make this
        // write idempotent from HealthKit's point of view: if the same
        // write-queue item is written again (confirm POST failed after a
        // successful write, item stayed pending, next sync re-attempts it),
        // HealthKit replaces the existing sample sharing this identifier
        // instead of inserting a duplicate.
        let metadata: [String: Any] = [
            HKMetadataKeySyncIdentifier: syncIdentifier,
            HKMetadataKeySyncVersion: 1,
        ]
        let sample = HKQuantitySample(
            type: quantityType,
            quantity: quantity,
            start: start,
            end: end,
            metadata: metadata
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
                // Same ADR-0008 cycle-prevention filter as the anchored
                // query: without it, every OwnPulse write-back wakes the
                // app via background delivery and kicks off a no-op sync
                // for a sample the app itself just wrote.
                let query = HKObserverQuery(sampleType: sampleType, predicate: Self.makeReadPredicate()) { _, completionHandler, error in
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
