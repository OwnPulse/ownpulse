// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit

// MARK: - Value types

/// Aggregated sleep data for a single night, derived from raw HealthKit samples.
struct HealthKitSleepSample: Sendable {
    let sleepStart: Date
    let sleepEnd: Date
    let durationMinutes: Int
    let deepMinutes: Int?
    let lightMinutes: Int?
    let remMinutes: Int?
    let awakeMinutes: Int?
    let sourceId: String?
}

/// A lightweight representation of a single raw HealthKit sleep sample,
/// decoupled from `HKCategorySample` so the aggregation logic can be tested
/// without a live `HKHealthStore`.
struct RawSleepSample: Sendable {
    let startDate: Date
    let endDate: Date
    /// Raw value of `HKCategoryValueSleepAnalysis`.
    let stage: Int
    let sourceBundleIdentifier: String
}

// MARK: - Protocol

/// Abstracts all HealthKit sleep access. Views and services never call HK directly.
protocol HealthKitSleepProvider: Sendable {
    /// Requests read authorization for sleep data. Must be called before any query.
    /// Throws `AppError.healthKitAuthorizationDenied` if the user denies access.
    func requestAuthorization() async throws

    /// Returns aggregated sleep samples for the given date range.
    func querySleepSamples(from start: Date, to end: Date) async throws -> [HealthKitSleepSample]
}

// MARK: - Live implementation

/// Production implementation backed by `HKHealthStore`.
final class LiveHealthKitSleepProvider: HealthKitSleepProvider, @unchecked Sendable {

    private let store: HKHealthStore

    /// The app's own bundle ID. Samples written by this bundle are filtered out
    /// unconditionally to prevent healthkit write-back cycles.
    private let ownBundleID: String

    init(store: HKHealthStore = HKHealthStore(), ownBundleID: String = Bundle.main.bundleIdentifier ?? "") {
        self.store = store
        self.ownBundleID = ownBundleID
    }

    // MARK: HealthKitSleepProvider

    func requestAuthorization() async throws {
        guard HKHealthStore.isHealthDataAvailable() else {
            throw AppError.healthKitNotAvailable
        }
        let sleepType = HKCategoryType(.sleepAnalysis)
        try await store.requestAuthorization(toShare: [], read: [sleepType])
    }

    func querySleepSamples(from start: Date, to end: Date) async throws -> [HealthKitSleepSample] {
        let sleepType = HKCategoryType(.sleepAnalysis)
        let predicate = HKQuery.predicateForSamples(withStart: start, end: end, options: .strictStartDate)

        let hkSamples: [HKCategorySample] = try await withCheckedThrowingContinuation { continuation in
            let query = HKSampleQuery(
                sampleType: sleepType,
                predicate: predicate,
                limit: HKObjectQueryNoLimit,
                sortDescriptors: [NSSortDescriptor(key: HKSampleSortIdentifierStartDate, ascending: true)]
            ) { _, samples, error in
                if let error {
                    continuation.resume(throwing: AppError.healthKitQueryFailed(error))
                    return
                }
                let typed = (samples as? [HKCategorySample]) ?? []
                continuation.resume(returning: typed)
            }
            store.execute(query)
        }

        // Convert to RawSleepSample and delegate to the static aggregator.
        // The static method handles the unconditional bundle ID exclusion.
        let raw = hkSamples.map { s in
            RawSleepSample(
                startDate: s.startDate,
                endDate: s.endDate,
                stage: s.value,
                sourceBundleIdentifier: s.sourceRevision.source.bundleIdentifier
            )
        }

        return Self.aggregate(raw, excludingBundleID: ownBundleID)
    }

    // MARK: - Testable static aggregation

    /// Groups `samples` by noon-to-noon night windows, drops any sample whose
    /// `sourceBundleIdentifier` matches `excludingBundleID` UNCONDITIONALLY
    /// (cycle-prevention guard), and aggregates stage minutes per night.
    ///
    /// This is `static` so tests can call it directly with `RawSleepSample`
    /// values, exercising the real logic without a live `HKHealthStore`.
    static func aggregate(_ samples: [RawSleepSample], excludingBundleID: String) -> [HealthKitSleepSample] {
        // UNCONDITIONAL: drop samples written by this app to prevent write-back cycles.
        let filtered = samples.filter { $0.sourceBundleIdentifier != excludingBundleID }

        guard !filtered.isEmpty else { return [] }

        // Group by night: noon-to-noon windows keyed by the anchor date string.
        let calendar = Calendar.current
        var nights: [String: [RawSleepSample]] = [:]

        for sample in filtered {
            let key = noonAnchorKey(for: sample.startDate, calendar: calendar)
            nights[key, default: []].append(sample)
        }

        var result: [HealthKitSleepSample] = []
        for (_, nightSamples) in nights {
            guard let aggregated = buildSample(from: nightSamples) else { continue }
            result.append(aggregated)
        }

        return result.sorted { $0.sleepStart < $1.sleepStart }
    }

    // MARK: - Private static helpers

    /// Returns a stable string key for the noon-to-noon window containing `date`.
    /// Dates before noon belong to the window starting noon the previous day.
    private static func noonAnchorKey(for date: Date, calendar: Calendar) -> String {
        let components = calendar.dateComponents([.year, .month, .day, .hour], from: date)
        let hour = components.hour ?? 0
        if hour < 12 {
            // Before noon — belongs to the previous day's window.
            guard let previous = calendar.date(byAdding: .day, value: -1, to: date) else {
                guard let y = components.year, let m = components.month, let d = components.day else {
                    return "unknown"
                }
                return "\(y)-\(m)-\(d)"
            }
            let prev = calendar.dateComponents([.year, .month, .day], from: previous)
            guard let y = prev.year, let m = prev.month, let d = prev.day else {
                return "unknown"
            }
            return "\(y)-\(m)-\(d)"
        }
        guard let y = components.year, let m = components.month, let d = components.day else {
            return "unknown"
        }
        return "\(y)-\(m)-\(d)"
    }

    private static func buildSample(from samples: [RawSleepSample]) -> HealthKitSleepSample? {
        guard let earliest = samples.map(\.startDate).min(),
              let latest = samples.map(\.endDate).max() else {
            return nil
        }

        var deepSeconds: Double = 0
        var lightSeconds: Double = 0
        var remSeconds: Double = 0
        var awakeSeconds: Double = 0

        for sample in samples {
            let duration = sample.endDate.timeIntervalSince(sample.startDate)
            guard duration > 0 else { continue }
            guard let value = HKCategoryValueSleepAnalysis(rawValue: sample.stage) else { continue }

            switch value {
            case .asleepDeep:
                deepSeconds += duration
            case .asleepLight:
                lightSeconds += duration
            case .asleepREM:
                remSeconds += duration
            case .awake:
                awakeSeconds += duration
            default:
                // .asleepUnspecified and any future cases contribute to total duration
                // but are not broken out into specific stage buckets.
                break
            }
        }

        let totalAsleep = deepSeconds + lightSeconds + remSeconds
        let durationSeconds = totalAsleep > 0 ? totalAsleep : latest.timeIntervalSince(earliest)
        let durationMinutes = max(1, Int(durationSeconds / 60))

        let sourceId = samples.first?.sourceBundleIdentifier

        return HealthKitSleepSample(
            sleepStart: earliest,
            sleepEnd: latest,
            durationMinutes: durationMinutes,
            deepMinutes: deepSeconds > 0 ? Int(deepSeconds / 60) : nil,
            lightMinutes: lightSeconds > 0 ? Int(lightSeconds / 60) : nil,
            remMinutes: remSeconds > 0 ? Int(remSeconds / 60) : nil,
            awakeMinutes: awakeSeconds > 0 ? Int(awakeSeconds / 60) : nil,
            sourceId: sourceId
        )
    }
}

// MARK: - AppError cases used here

// AppError is defined in the main app target. These cases must exist there:
//   case healthKitNotAvailable
//   case healthKitAuthorizationDenied
//   case healthKitQueryFailed(Error)
// They are referenced here so that callers can match on them.
