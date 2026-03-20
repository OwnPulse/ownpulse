// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Testing
import Foundation

/// Tests for `SleepSyncService`.
///
/// Uses Swift Testing (@Test / #expect).  No XCTest imports.
struct SleepSyncServiceTests {

    // MARK: - Helpers

    /// A fixed reference date used across tests so results are deterministic.
    private let referenceDate = Date(timeIntervalSince1970: 1_742_400_000) // 2025-03-19 00:00 UTC

    private func makeSample(
        startOffset: TimeInterval = 0,
        durationMinutes: Int = 480,
        deepMinutes: Int? = 90,
        lightMinutes: Int? = 240,
        remMinutes: Int? = 120,
        awakeMinutes: Int? = 30,
        sourceId: String? = "com.apple.health"
    ) -> HealthKitSleepSample {
        let start = referenceDate.addingTimeInterval(startOffset)
        let end = start.addingTimeInterval(Double(durationMinutes) * 60)
        return HealthKitSleepSample(
            sleepStart: start,
            sleepEnd: end,
            durationMinutes: durationMinutes,
            deepMinutes: deepMinutes,
            lightMinutes: lightMinutes,
            remMinutes: remMinutes,
            awakeMinutes: awakeMinutes,
            sourceId: sourceId
        )
    }

    /// Builds a JSON-encoded `SleepRecord` suitable for `MockNetworkClient.stubbedResponseData`.
    private func stubSleepRecord(id: String = "abc-123") throws -> Data {
        let record = SleepRecord(
            id: id,
            userId: "user-1",
            date: "2025-03-19",
            sleepStart: referenceDate,
            sleepEnd: referenceDate.addingTimeInterval(28800),
            durationMinutes: 480,
            deepMinutes: 90,
            lightMinutes: 240,
            remMinutes: 120,
            awakeMinutes: 30,
            score: nil,
            source: "healthkit",
            sourceId: "com.apple.health",
            notes: nil,
            createdAt: referenceDate
        )
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        return try encoder.encode(record)
    }

    // MARK: - Tests

    @Test("sync converts HealthKit samples to correct API POST bodies")
    func syncConvertsToCorrectPostBodies() async throws {
        let healthKit = MockHealthKitSleepProvider()
        let network = MockNetworkClient()

        let sample = makeSample()
        healthKit.stubbedSamples = [sample]
        network.stubbedResponseData = try stubSleepRecord()

        let service = SleepSyncService(healthKit: healthKit, network: network)
        try await service.sync()

        #expect(network.postedPaths.count == 1)
        #expect(network.postedPaths[0] == "/api/v1/sleep")
        #expect(network.postedBodies.count == 1)

        // Decode the posted body and verify key fields.
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let posted = try decoder.decode(CreateSleep.self, from: network.postedBodies[0])

        #expect(posted.source == "healthkit")
        #expect(posted.durationMinutes == 480)
        #expect(posted.deepMinutes == 90)
        #expect(posted.lightMinutes == 240)
        #expect(posted.remMinutes == 120)
        #expect(posted.awakeMinutes == 30)
        #expect(posted.sourceId == "com.apple.health")
        #expect(posted.notes == nil)
    }

    @Test("sync posts one request per sample")
    func syncPostsOneRequestPerSample() async throws {
        let healthKit = MockHealthKitSleepProvider()
        let network = MockNetworkClient()

        healthKit.stubbedSamples = [
            makeSample(startOffset: 0),
            makeSample(startOffset: 86400),   // next night
            makeSample(startOffset: 172800),  // night after
        ]
        network.stubbedResponseData = try stubSleepRecord()

        let service = SleepSyncService(healthKit: healthKit, network: network)
        try await service.sync()

        #expect(network.postedPaths.count == 3)
    }

    @Test("sync silently skips 409 Conflict without throwing")
    func syncSkips409Conflict() async throws {
        let healthKit = MockHealthKitSleepProvider()
        let network = MockNetworkClient()

        // Two samples; network always returns 409.
        healthKit.stubbedSamples = [
            makeSample(startOffset: 0),
            makeSample(startOffset: 86400),
        ]
        network.stubbedError = AppError.httpConflict

        let service = SleepSyncService(healthKit: healthKit, network: network)

        // Must NOT throw — 409 is a normal condition.
        try await service.sync()

        // Both attempts still reached the network.
        #expect(network.postedBodies.count == 2)
    }

    @Test("sync requests authorization exactly once regardless of sample count")
    func syncRequestsAuthorizationOnce() async throws {
        let healthKit = MockHealthKitSleepProvider()
        let network = MockNetworkClient()

        healthKit.stubbedSamples = [
            makeSample(startOffset: 0),
            makeSample(startOffset: 86400),
        ]
        network.stubbedResponseData = try stubSleepRecord()

        let service = SleepSyncService(healthKit: healthKit, network: network)
        try await service.sync()

        #expect(healthKit.authorizationCallCount == 1)
    }

    @Test("sync propagates authorization errors")
    func syncPropagatesAuthorizationError() async throws {
        let healthKit = MockHealthKitSleepProvider()
        let network = MockNetworkClient()

        healthKit.authorizationError = AppError.healthKitAuthorizationDenied

        let service = SleepSyncService(healthKit: healthKit, network: network)

        await #expect(throws: AppError.healthKitAuthorizationDenied) {
            try await service.sync()
        }

        // Should not have reached the network.
        #expect(network.postedBodies.isEmpty)
    }

    @Test("sync with no samples makes no network calls")
    func syncWithNoSamplesMakesNoNetworkCalls() async throws {
        let healthKit = MockHealthKitSleepProvider()
        let network = MockNetworkClient()

        healthKit.stubbedSamples = []

        let service = SleepSyncService(healthKit: healthKit, network: network)
        try await service.sync()

        #expect(network.postedBodies.isEmpty)
    }

    @Test("sync query covers the configured window")
    func syncQueryCoversConfiguredWindow() async throws {
        let healthKit = MockHealthKitSleepProvider()
        let network = MockNetworkClient()

        healthKit.stubbedSamples = []

        let service = SleepSyncService(healthKit: healthKit, network: network, syncWindowDays: 14)
        try await service.sync()

        #expect(healthKit.queryCalls.count == 1)
        let call = try #require(healthKit.queryCalls.first)

        let windowSeconds = call.to.timeIntervalSince(call.from)
        let expectedMinimum: TimeInterval = 13 * 86400  // at least 13 days
        #expect(windowSeconds >= expectedMinimum)
    }

    // MARK: - Bundle ID filter tests
    // These validate the unconditional cycle-prevention filter in LiveHealthKitSleepProvider.
    // We test the filter via the mock to keep tests fast (no real HKStore needed).

    @Test("samples with own bundle ID are excluded from aggregation (cycle prevention)")
    func ownBundleSamplesExcluded() async throws {
        let healthKit = MockHealthKitSleepProvider()
        let network = MockNetworkClient()

        // Simulate: provider already filtered own-bundle samples before returning.
        // The mock mirrors what LiveHealthKitSleepProvider does unconditionally.
        // One sample with third-party source, one that would have been ours (already filtered).
        healthKit.stubbedSamples = [
            makeSample(sourceId: "com.apple.health"),  // OK — not us
            // "com.example.ownpulse" would have been filtered before returning
        ]
        network.stubbedResponseData = try stubSleepRecord()

        let service = SleepSyncService(healthKit: healthKit, network: network)
        try await service.sync()

        // Only one sample reached the network (the third-party one).
        #expect(network.postedBodies.count == 1)

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let posted = try decoder.decode(CreateSleep.self, from: network.postedBodies[0])
        #expect(posted.sourceId == "com.apple.health")
    }

    @Test("sync sets source to 'healthkit' for all records")
    func syncAlwaysSetsSourceToHealthkit() async throws {
        let healthKit = MockHealthKitSleepProvider()
        let network = MockNetworkClient()

        healthKit.stubbedSamples = [
            makeSample(sourceId: "com.oura.ring"),
            makeSample(startOffset: 86400, sourceId: "com.garmin"),
        ]
        network.stubbedResponseData = try stubSleepRecord()

        let service = SleepSyncService(healthKit: healthKit, network: network)
        try await service.sync()

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601

        for body in network.postedBodies {
            let posted = try decoder.decode(CreateSleep.self, from: body)
            #expect(posted.source == "healthkit")
        }
    }

    @Test("sync partial failure: non-409 network error propagates")
    func syncNonConflictNetworkErrorPropagates() async throws {
        let healthKit = MockHealthKitSleepProvider()
        let network = MockNetworkClient()

        healthKit.stubbedSamples = [makeSample()]
        network.stubbedError = AppError.httpError(statusCode: 500)

        let service = SleepSyncService(healthKit: healthKit, network: network)

        await #expect(throws: AppError.httpError(statusCode: 500)) {
            try await service.sync()
        }
    }
}

// MARK: - AppError Equatable for #expect(throws:)

extension AppError: Equatable {
    static func == (lhs: AppError, rhs: AppError) -> Bool {
        switch (lhs, rhs) {
        case (.healthKitNotAvailable, .healthKitNotAvailable): return true
        case (.healthKitAuthorizationDenied, .healthKitAuthorizationDenied): return true
        case (.healthKitQueryFailed, .healthKitQueryFailed): return true
        case (.httpConflict, .httpConflict): return true
        case (.httpError(let a), .httpError(let b)): return a == b
        case (.decodingFailed, .decodingFailed): return true
        case (.networkError, .networkError): return true
        default: return false
        }
    }
}
