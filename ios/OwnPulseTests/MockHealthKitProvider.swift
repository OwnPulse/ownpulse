// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import HealthKit
@testable import OwnPulse

final class MockHealthKitProvider: HealthKitProviderProtocol, @unchecked Sendable {
    var authorizationRequested = false
    var isAuthorizedResult = true
    var mockSamples: [HealthKitSample] = []
    var mockAnchor: Data?
    var writtenSamples: [(type: HKSampleType, value: Double, unit: HKUnit, start: Date, end: Date)] = []

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
}
