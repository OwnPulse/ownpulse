// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

/// Test double for `HealthKitSleepProvider`.
/// Pre-load `stubbedSamples` before the call under test; inspect `calls` after.
final class MockHealthKitSleepProvider: HealthKitSleepProvider, @unchecked Sendable {

    // MARK: - Stub configuration

    /// Samples returned by `querySleepSamples`. Replace in each test.
    var stubbedSamples: [HealthKitSleepSample] = []

    /// When non-nil, `requestAuthorization()` throws this error instead of succeeding.
    var authorizationError: (any Error)?

    /// When non-nil, `querySleepSamples` throws this error instead of returning `stubbedSamples`.
    var queryError: (any Error)?

    // MARK: - Call tracking

    private(set) var authorizationCallCount = 0
    private(set) var queryCalls: [(from: Date, to: Date)] = []

    // MARK: - HealthKitSleepProvider

    func requestAuthorization() async throws {
        authorizationCallCount += 1
        if let error = authorizationError {
            throw error
        }
    }

    func querySleepSamples(from start: Date, to end: Date) async throws -> [HealthKitSleepSample] {
        queryCalls.append((from: start, to: end))
        if let error = queryError {
            throw error
        }
        return stubbedSamples
    }
}
