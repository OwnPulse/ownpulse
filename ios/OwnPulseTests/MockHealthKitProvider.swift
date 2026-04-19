// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit
@testable import OwnPulse

final class MockHealthKitProvider: HealthKitProviderProtocol, @unchecked Sendable {
    private let lock = NSLock()

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
        lock.lock(); defer { lock.unlock() }
        return _observerStartCount
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
        lock.lock()
        _observerStartCount += 1
        lock.unlock()

        return AsyncStream { continuation in
            self.setContinuation(continuation)
            continuation.onTermination = { [weak self] _ in
                self?.setContinuation(nil)
            }
        }
    }

    func enableBackgroundDelivery() async throws {
        lock.lock()
        backgroundDeliveryCallCount += 1
        let shouldThrow = backgroundDeliveryError
        lock.unlock()

        if let error = shouldThrow {
            throw error
        }

        lock.lock()
        backgroundDeliveryEnabled = true
        backgroundDeliveryDisabled = false
        lock.unlock()
    }

    func disableAllBackgroundDelivery() async throws {
        lock.lock()
        disableBackgroundDeliveryCallCount += 1
        let shouldThrow = disableBackgroundDeliveryError
        lock.unlock()

        if let error = shouldThrow {
            throw error
        }

        lock.lock()
        backgroundDeliveryDisabled = true
        backgroundDeliveryEnabled = false
        lock.unlock()
    }

    // MARK: - Test driver

    /// Simulate HealthKit firing the observer query. Tests call this to drive
    /// the subscription logic in `SyncCoordinator`.
    func fireObserver() {
        lock.lock()
        let cont = observerContinuation
        lock.unlock()
        cont?.yield()
    }

    /// Signal that the observer stream has ended (e.g. on logout).
    func endObserver() {
        lock.lock()
        let cont = observerContinuation
        lock.unlock()
        cont?.finish()
    }

    // MARK: - Private

    private func setContinuation(_ continuation: AsyncStream<Void>.Continuation?) {
        lock.lock()
        observerContinuation = continuation
        lock.unlock()
    }
}
