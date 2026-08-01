// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import BackgroundTasks
import Foundation
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "sync-scheduler")

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

    /// Submits a BGAppRefresh request. Submission failures are expected in
    /// some test hosts (no BGTaskScheduler entitlement) and the next sync
    /// attempt will try again, so this never throws — but on device, a
    /// failure here (e.g. an identifier not present in
    /// `BGTaskSchedulerPermittedIdentifiers`) would otherwise be completely
    /// unobservable, so log it.
    func scheduleNextSync() {
        let request = BGAppRefreshTaskRequest(identifier: Self.taskIdentifier)
        request.earliestBeginDate = Date(timeIntervalSinceNow: Self.earliestDelaySeconds)
        do {
            try submitter.submit(request)
        } catch {
            logger.error("Failed to submit BGAppRefreshTaskRequest: \(error.localizedDescription, privacy: .public)")
        }
    }
}
