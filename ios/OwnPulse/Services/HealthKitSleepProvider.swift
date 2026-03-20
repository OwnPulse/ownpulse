// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit

// MARK: - Value type

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

        let rawSamples: [HKCategorySample] = try await withCheckedThrowingContinuation { continuation in
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

        // UNCONDITIONAL: drop any samples whose source bundle ID matches ours.
        // This prevents write-back cycles regardless of any configuration.
        let filtered = rawSamples.filter { $0.sourceRevision.source.bundleIdentifier != ownBundleID }

        return aggregate(samples: filtered)
    }

    // MARK: - Private helpers

    /// Groups samples by noon-to-noon night windows and aggregates stage minutes.
    private func aggregate(samples: [HKCategorySample]) -> [HealthKitSleepSample] {
        guard !samples.isEmpty else { return [] }

        // Group samples by night key: the calendar date of the noon anchor that
        // precedes each sample's start time.  A sample starting before noon belongs
        // to the night whose noon anchor is the previous day.
        var nights: [String: [HKCategorySample]] = [:]
        let calendar = Calendar.current

        for sample in samples {
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

    /// Returns a stable string key for the noon-to-noon window containing `date`.
    /// Dates before noon belong to the window starting noon the previous day.
    private func noonAnchorKey(for date: Date, calendar: Calendar) -> String {
        var components = calendar.dateComponents([.year, .month, .day, .hour], from: date)
        let hour = components.hour ?? 0
        if hour < 12 {
            // Before noon — belongs to the previous day's window
            guard let previous = calendar.date(byAdding: .day, value: -1, to: date) else {
                return "\(components.year!)-\(components.month!)-\(components.day!)"
            }
            let prev = calendar.dateComponents([.year, .month, .day], from: previous)
            return "\(prev.year!)-\(prev.month!)-\(prev.day!)"
        }
        components.hour = nil
        return "\(components.year!)-\(components.month!)-\(components.day!)"
    }

    private func buildSample(from samples: [HKCategorySample]) -> HealthKitSleepSample? {
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
            guard let value = HKCategoryValueSleepAnalysis(rawValue: sample.value) else { continue }

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

        let sourceId = samples.first.map { $0.sourceRevision.source.bundleIdentifier }

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
