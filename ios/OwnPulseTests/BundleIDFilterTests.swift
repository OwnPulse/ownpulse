// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Testing
import Foundation
import HealthKit

/// Tests for `LiveHealthKitSleepProvider.aggregate(_:excludingBundleID:)`.
///
/// These tests call the static method directly — no `HKHealthStore` or
/// simulator required.  They verify the unconditional cycle-prevention rule:
/// samples whose `sourceBundleIdentifier` matches the app's own bundle ID are
/// ALWAYS excluded, regardless of stage, time, or any other property.
struct BundleIDFilterTests {

    // MARK: - Helpers

    private let ownBundle = "com.example.ownpulse"
    private let thirdParty = "com.apple.health"
    private let anotherThirdParty = "com.oura.ring"

    /// 2026-03-19 22:00 UTC — a typical sleep start time.
    private let sleepStart = Date(timeIntervalSince1970: 1_742_425_200)

    /// Stage raw values for the cases used in tests.
    /// `HKCategoryValueSleepAnalysis.asleepLight` raw value is 1 on current SDK.
    private var stageLight: Int { HKCategoryValueSleepAnalysis.asleepLight.rawValue }
    private var stageDeep: Int { HKCategoryValueSleepAnalysis.asleepDeep.rawValue }
    private var stageREM: Int { HKCategoryValueSleepAnalysis.asleepREM.rawValue }

    private func raw(
        bundleID: String,
        startOffset: TimeInterval = 0,
        durationSeconds: TimeInterval = 3600,
        stage: Int? = nil
    ) -> RawSleepSample {
        let start = sleepStart.addingTimeInterval(startOffset)
        return RawSleepSample(
            startDate: start,
            endDate: start.addingTimeInterval(durationSeconds),
            stage: stage ?? stageLight,
            sourceBundleIdentifier: bundleID
        )
    }

    // MARK: - Tests

    @Test("own bundle ID samples are excluded unconditionally — no output when all samples are ours")
    func allOwnBundleSamplesExcluded() {
        let samples = [
            raw(bundleID: ownBundle, startOffset: 0),
            raw(bundleID: ownBundle, startOffset: 3600),
            raw(bundleID: ownBundle, startOffset: 7200),
        ]

        let result = LiveHealthKitSleepProvider.aggregate(samples, excludingBundleID: ownBundle)

        #expect(result.isEmpty)
    }

    @Test("third-party samples pass through when none belong to own bundle")
    func thirdPartySamplesPassThrough() {
        let samples = [
            raw(bundleID: thirdParty, startOffset: 0, stage: stageLight),
            raw(bundleID: thirdParty, startOffset: 3600, stage: stageDeep),
        ]

        let result = LiveHealthKitSleepProvider.aggregate(samples, excludingBundleID: ownBundle)

        #expect(result.count == 1)
        #expect(result[0].lightMinutes != nil)
        #expect(result[0].deepMinutes != nil)
    }

    @Test("mixed bundle IDs: own-bundle samples excluded, third-party samples kept")
    func mixedBundleIDsOwnExcluded() {
        let samples = [
            raw(bundleID: thirdParty, startOffset: 0, stage: stageLight),
            raw(bundleID: ownBundle, startOffset: 3600, stage: stageDeep),   // must be excluded
            raw(bundleID: thirdParty, startOffset: 7200, stage: stageREM),
        ]

        let result = LiveHealthKitSleepProvider.aggregate(samples, excludingBundleID: ownBundle)

        // All three samples fall in the same noon-to-noon window, so they group
        // into one night.  Only the two third-party samples should contribute.
        #expect(result.count == 1)
        let night = result[0]

        // deep_minutes must be nil because the deep sample was from ownBundle and was excluded.
        #expect(night.deepMinutes == nil)
        // light and REM come from the two kept samples.
        #expect(night.lightMinutes != nil)
        #expect(night.remMinutes != nil)
    }

    @Test("own bundle exclusion is unconditional — applies regardless of stage value")
    func exclusionIsUnconditionalAcrossAllStages() {
        let stages = [stageLight, stageDeep, stageREM,
                      HKCategoryValueSleepAnalysis.awake.rawValue,
                      HKCategoryValueSleepAnalysis.asleepUnspecified.rawValue]

        for stage in stages {
            let samples = [raw(bundleID: ownBundle, stage: stage)]
            let result = LiveHealthKitSleepProvider.aggregate(samples, excludingBundleID: ownBundle)
            #expect(result.isEmpty, "Stage \(stage) from own bundle should be excluded")
        }
    }

    @Test("samples from two different third-party sources are both kept")
    func multiplThirdPartySourcesBothKept() {
        // Two nights: one from each third-party source, 24 h apart.
        let samples = [
            raw(bundleID: thirdParty, startOffset: 0),
            raw(bundleID: anotherThirdParty, startOffset: 86400),
        ]

        let result = LiveHealthKitSleepProvider.aggregate(samples, excludingBundleID: ownBundle)

        #expect(result.count == 2)
    }

    @Test("empty input returns empty output")
    func emptyInputReturnsEmpty() {
        let result = LiveHealthKitSleepProvider.aggregate([], excludingBundleID: ownBundle)
        #expect(result.isEmpty)
    }
}
