// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import BackgroundTasks
import Foundation

/// Minimal abstraction over `BGTaskScheduler.submit` so tests can observe
/// scheduling without a real `BGTaskScheduler` (which cannot be stubbed
/// directly and raises in the unit-test host).
protocol BackgroundTaskSubmitter: Sendable {
    func submit(_ request: BGTaskRequest) throws
}

/// Default implementation that forwards to the system scheduler.
struct SystemBackgroundTaskSubmitter: BackgroundTaskSubmitter {
    func submit(_ request: BGTaskRequest) throws {
        try BGTaskScheduler.shared.submit(request)
    }
}

final class SyncScheduler: Sendable {
    static let taskIdentifier = "health.ownpulse.sync"

    /// Earliest time (in seconds) from now at which iOS may wake us for a
    /// sync. iOS treats this as a hint, not a commitment; the actual wake
    /// can be minutes later depending on system conditions.
    static let earliestDelaySeconds: TimeInterval = 15 * 60

    private let submitter: BackgroundTaskSubmitter

    init(submitter: BackgroundTaskSubmitter = SystemBackgroundTaskSubmitter()) {
        self.submitter = submitter
    }

    /// Submits a BGAppRefresh request. Failures are swallowed — we can't
    /// schedule in some test hosts (no entitlement) and the next sync
    /// attempt will try again.
    func scheduleNextSync() {
        let request = BGAppRefreshTaskRequest(identifier: Self.taskIdentifier)
        request.earliestBeginDate = Date(timeIntervalSinceNow: Self.earliestDelaySeconds)
        try? submitter.submit(request)
    }
}
