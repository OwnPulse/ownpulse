// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

/// Drives the HealthKit → backend sleep sync flow.
///
/// Usage:
/// ```swift
/// let service = SleepSyncService(healthKit: LiveHealthKitSleepProvider(), network: liveClient)
/// try await service.sync()
/// ```
///
/// - HealthKit authorization is requested at the point of sync (incremental permission).
/// - No data is transmitted until `sync()` is explicitly called by the user.
/// - 409 Conflict responses are silently ignored (record already exists server-side).
final class SleepSyncService: Sendable {

    private let healthKit: any HealthKitSleepProvider
    private let network: any NetworkClient

    /// How many days back to sync.  Kept small to bound request volume.
    private let syncWindowDays: Int

    init(
        healthKit: any HealthKitSleepProvider,
        network: any NetworkClient,
        syncWindowDays: Int = 7
    ) {
        self.healthKit = healthKit
        self.network = network
        self.syncWindowDays = syncWindowDays
    }

    // MARK: - Public API

    /// Fetches sleep samples for the last `syncWindowDays` days and POSTs each to
    /// `/api/v1/sleep`.  Must be called on a task that can tolerate suspension.
    ///
    /// - Throws: `AppError` for HealthKit failures or unrecoverable network errors.
    ///           409 Conflict is NOT thrown — it is silently skipped.
    func sync() async throws {
        // Request permission at the moment the user initiates sync (incremental).
        try await healthKit.requestAuthorization()

        let end = Date()
        let start = Calendar.current.date(byAdding: .day, value: -syncWindowDays, to: end) ?? end
        let samples = try await healthKit.querySleepSamples(from: start, to: end)

        for sample in samples {
            let body = CreateSleep(
                date: isoDateString(from: sample.sleepStart),
                sleepStart: sample.sleepStart,
                sleepEnd: sample.sleepEnd,
                durationMinutes: sample.durationMinutes,
                deepMinutes: sample.deepMinutes,
                lightMinutes: sample.lightMinutes,
                remMinutes: sample.remMinutes,
                awakeMinutes: sample.awakeMinutes,
                score: nil,
                source: "healthkit",
                sourceId: sample.sourceId,
                notes: nil
            )

            do {
                let _: SleepRecord = try await network.post("/api/v1/sleep", body: body)
            } catch AppError.httpConflict {
                // Record already exists — not an error condition.
                continue
            }
        }
    }

    // MARK: - Private helpers

    private func isoDateString(from date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.timeZone = TimeZone.current
        return formatter.string(from: date)
    }
}
