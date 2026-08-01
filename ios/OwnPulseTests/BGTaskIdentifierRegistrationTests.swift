// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

/// Guards against a repeat of the bug where `OwnPulseApp` registered a
/// hardcoded `"health.ownpulse.sync"` string with `BGTaskScheduler` instead
/// of `SyncScheduler.taskIdentifier`. If the two ever drift apart,
/// `BGTaskScheduler.register(forTaskWithIdentifier:)` — which requires the
/// identifier to be present in `Info.plist`'s
/// `BGTaskSchedulerPermittedIdentifiers` — starts silently failing (or the
/// task the app *schedules* via `SyncScheduler` no longer matches anything
/// it registered a handler for).
@Suite("BGTask identifier stays in sync with Info.plist")
struct BGTaskIdentifierRegistrationTests {
    @Test("SyncScheduler.taskIdentifier is listed in Info.plist's BGTaskSchedulerPermittedIdentifiers")
    func taskIdentifierMatchesInfoPlist() throws {
        // `AppDependencies` is a class defined in the OwnPulse app target
        // (not this test target), so `Bundle(for:)` resolves to the app
        // bundle whose `Info.plist` we need to inspect.
        let bundle = Bundle(for: AppDependencies.self)
        let permittedIdentifiers = try #require(
            bundle.object(forInfoDictionaryKey: "BGTaskSchedulerPermittedIdentifiers") as? [String]
        )

        #expect(
            permittedIdentifiers.contains(SyncScheduler.taskIdentifier),
            "Info.plist BGTaskSchedulerPermittedIdentifiers \(permittedIdentifiers) must contain " +
            "SyncScheduler.taskIdentifier (\(SyncScheduler.taskIdentifier)) or background sync " +
            "registration silently fails on device."
        )
    }
}
