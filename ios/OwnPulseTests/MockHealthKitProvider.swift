// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit
import os
@testable import OwnPulse

final class MockHealthKitProvider: HealthKitProviderProtocol, @unchecked Sendable {
    // Swift 6 flags `NSLock.lock()` as unavailable from async contexts. The
    // mock is touched from both the test body and from `SyncCoordinator`
    // (which is an actor that hops off MainActor), so a real mutex is still
    // required — but `OSAllocatedUnfairLock.withLock` is the async-safe
    // alternative: it's synchronous, runs the closure under the lock, and
    // never suspends.
    private let lock = OSAllocatedUnfairLock()

    var authorizationRequested = false
    var isAuthorizedResult = true
    var mockSamples: [HealthKitSample] = []
    var mockAnchor: Data?
    var writtenSamples: [(type: HKSampleType, value: Double, unit: HKUnit, start: Date, end: Date)] = []

    // Background delivery hooks for tests.
    private(set) var backgroundDeliveryEnabled = false
    private(set) var backgroundDeliveryCallCount = 0
    private(set) var backgroundDeliveryDisabled = false
    private(set) var disableBackgroundDeliveryCallCount = 0
    var backgroundDeliveryError: Error?
    var disableBackgroundDeliveryError: Error?

    // Observer stream wiring: tests drive updates through `fireObserver()`.
    // Guarded by `lock` because the coordinator's actor and the test body
    // can touch the continuation from different queues.
    private var observerContinuation: AsyncStream<Void>.Continuation?
    private var _observerStartCount = 0

    var observerStartCount: Int {
        lock.withLock { _observerStartCount }
    }

    func requestAuthorization() async throws {
        authorizationRequested = true
    }

    func isAuthorized() -> Bool {
        isAuthorizedResult
    }

    func querySamples(
        type: HKSampleType,
        anchor: Data?
    ) async throws -> AnchoredQueryResult {
        AnchoredQueryResult(
            samples: mockSamples,
            newAnchor: mockAnchor,
            deletedObjectIDs: []
        )
    }

    func writeSample(
        type: HKSampleType,
        value: Double,
        unit: HKUnit,
        start: Date,
        end: Date
    ) async throws {
        writtenSamples.append((type: type, value: value, unit: unit, start: start, end: end))
    }

    func observeSampleUpdates() -> AsyncStream<Void> {
        lock.withLock {
            _observerStartCount += 1
        }

        return AsyncStream { continuation in
            self.setContinuation(continuation)
            continuation.onTermination = { [weak self] _ in
                self?.setContinuation(nil)
            }
        }
    }

    func enableBackgroundDelivery() async throws {
        let shouldThrow: Error? = lock.withLock {
            backgroundDeliveryCallCount += 1
            return backgroundDeliveryError
        }

        if let error = shouldThrow {
            throw error
        }

        lock.withLock {
            backgroundDeliveryEnabled = true
            backgroundDeliveryDisabled = false
        }
    }

    func disableAllBackgroundDelivery() async throws {
        let shouldThrow: Error? = lock.withLock {
            disableBackgroundDeliveryCallCount += 1
            return disableBackgroundDeliveryError
        }

        if let error = shouldThrow {
            throw error
        }

        lock.withLock {
            backgroundDeliveryDisabled = true
            backgroundDeliveryEnabled = false
        }
    }

    // MARK: - Test driver

    /// Simulate HealthKit firing the observer query. Tests call this to drive
    /// the subscription logic in `SyncCoordinator`.
    func fireObserver() {
        let cont = lock.withLock { observerContinuation }
        cont?.yield()
    }

    /// Signal that the observer stream has ended (e.g. on logout).
    func endObserver() {
        let cont = lock.withLock { observerContinuation }
        cont?.finish()
    }

    // MARK: - Private

    private func setContinuation(_ continuation: AsyncStream<Void>.Continuation?) {
        lock.withLock {
            observerContinuation = continuation
        }
    }
}
