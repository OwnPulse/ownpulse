// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import BackgroundTasks
import Foundation
import Testing
@testable import OwnPulse

@Suite("SyncScheduler")
struct SyncSchedulerTests {
    @Test("scheduleNextSync submits a BGAppRefreshTaskRequest with the sync identifier")
    func schedulesRefreshRequest() {
        let submitter = RecordingSubmitter()
        let scheduler = SyncScheduler(submitter: submitter)

        scheduler.scheduleNextSync()

        #expect(submitter.submittedRequests.count == 1)

        let request = submitter.submittedRequests.first
        #expect(request?.identifier == SyncScheduler.taskIdentifier)
        #expect(request is BGAppRefreshTaskRequest)
    }

    @Test("scheduleNextSync sets earliestBeginDate ~15 minutes in the future")
    func earliestBeginDateIsFifteenMinutes() {
        let submitter = RecordingSubmitter()
        let scheduler = SyncScheduler(submitter: submitter)

        let before = Date()
        scheduler.scheduleNextSync()
        let after = Date()

        let request = submitter.submittedRequests.first
        let earliest = request?.earliestBeginDate ?? .distantPast

        // Allow a small window: the earliest begin date should fall within
        // [before + 15min, after + 15min]. Using >=/<= to avoid flakes on
        // slow CI.
        let minExpected = before.addingTimeInterval(SyncScheduler.earliestDelaySeconds)
        let maxExpected = after.addingTimeInterval(SyncScheduler.earliestDelaySeconds)
        #expect(earliest >= minExpected)
        #expect(earliest <= maxExpected)
    }

    @Test("scheduleNextSync swallows submitter errors — caller never sees a throw")
    func swallowsErrors() {
        struct SubmitFail: Error {}
        let submitter = RecordingSubmitter()
        submitter.error = SubmitFail()

        let scheduler = SyncScheduler(submitter: submitter)

        // If scheduleNextSync rethrew we'd crash the test. The contract is
        // "best effort, retry later"; exercising that path here guards
        // against a regression that would surface as an unhandled throw in
        // production.
        scheduler.scheduleNextSync()

        #expect(submitter.submittedRequests.count == 1)
    }
}

/// Records the `BGTaskRequest` instances passed to `submit(_:)`. Can be
/// configured to throw to exercise error paths.
private final class RecordingSubmitter: BackgroundTaskSubmitter, @unchecked Sendable {
    var submittedRequests: [BGTaskRequest] = []
    var error: Error?

    func submit(_ request: BGTaskRequest) throws {
        submittedRequests.append(request)
        if let error { throw error }
    }
}
