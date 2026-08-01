// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

/// Asserts `SyncScheduler.taskIdentifier` is one of `Info.plist`'s
/// `BGTaskSchedulerPermittedIdentifiers`.
///
/// Note what this does *not* guard: it can't see what literal
/// `OwnPulseApp.registerBackgroundTasks()` actually passes to
/// `BGTaskScheduler.register(forTaskWithIdentifier:)` — there's no seam to
/// intercept that call from a unit test. If `OwnPulseApp` and
/// `SyncScheduler` ever register/schedule two different identifier
/// literals again, this test would still pass as long as both happen to be
/// listed in `Info.plist`. It only catches `SyncScheduler.taskIdentifier`
/// drifting away from what `Info.plist` permits, which is enough to catch
/// the specific class of bug (a permitted-identifiers entry going stale)
/// that `BGTaskScheduler.register` fails on unconditionally.
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
            "Info.plist BGTaskSchedulerPermittedIdentifiers \(permittedIdentifiers) must contain SyncScheduler.taskIdentifier (\(SyncScheduler.taskIdentifier)) or background sync registration silently fails on device."
        )
    }
}
