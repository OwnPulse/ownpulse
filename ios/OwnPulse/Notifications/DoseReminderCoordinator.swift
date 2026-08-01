// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "notifications")

/// Fetches the data `DoseReminderScheduler` needs and drives
/// `NotificationManagerProtocol`. Extracted as a protocol so `ProtocolsViewModel`
/// and `AppDependencies` can depend on it without pulling in networking or
/// notification internals, and so tests can substitute a spy.
protocol DoseReminderRebuilding: Sendable {
    /// Fetches the current active runs, resolves per-substance schedule data
    /// for each notify-enabled run, and rebuilds every pending local dose
    /// reminder to match. Safe to call anytime — no-ops gracefully on
    /// network failure (existing reminders are left as-is rather than
    /// cleared, since a fetch failure isn't evidence the run changed).
    func rebuildReminders() async
    /// Cancels every pending dose reminder. Used on logout.
    func clearAll() async
}

/// Local-only: this coordinator never sends anything to the backend beyond
/// the read-only GETs already used elsewhere to display protocols/runs. All
/// scheduling happens on-device via `UNCalendarNotificationTrigger`.
@MainActor
final class DoseReminderCoordinator: DoseReminderRebuilding {
    private let networkClient: NetworkClientProtocol
    private let notificationManager: NotificationManagerProtocol
    /// Re-checked after every await in `performRebuild()` — a rebuild that
    /// straddles a logout must not re-add reminders (with substance names on
    /// the lock screen) for a user who just signed out.
    private let isAuthenticated: @MainActor @Sendable () -> Bool

    /// The in-flight rebuild, if any. App-foreground and active-run-list
    /// refreshes (`ProtocolsViewModel.loadProtocols`) can both trigger a
    /// rebuild around the same time; without coalescing here, their fetch +
    /// remove/add cycles would interleave unpredictably against the same
    /// notification center. Latest call wins — starting a new rebuild
    /// cancels whatever's still in flight.
    private var inFlightTask: Task<Void, Never>?

    init(
        networkClient: NetworkClientProtocol,
        notificationManager: NotificationManagerProtocol,
        isAuthenticated: @escaping @MainActor @Sendable () -> Bool
    ) {
        self.networkClient = networkClient
        self.notificationManager = notificationManager
        self.isAuthenticated = isAuthenticated
    }

    func rebuildReminders() async {
        inFlightTask?.cancel()
        let task = Task { [weak self] in
            await self?.performRebuild()
        }
        inFlightTask = task
        await task.value
    }

    private func performRebuild() async {
        guard isAuthenticated(), !Task.isCancelled else { return }

        let runs: [ActiveRunResponse]
        do {
            runs = try await networkClient.request(
                method: "GET",
                path: Endpoints.activeRuns,
                body: nil as String?
            )
        } catch {
            logger.error("Failed to fetch active runs for dose reminders: \(error.localizedDescription, privacy: .public)")
            return
        }

        guard isAuthenticated(), !Task.isCancelled else { return }

        let notifyRuns = runs.filter(\.notify)
        var doseRuns: [DoseReminderRun] = []
        doseRuns.reserveCapacity(notifyRuns.count)

        for run in notifyRuns {
            guard !Task.isCancelled else { return }

            guard let startDate = Self.parseDate(run.startDate) else {
                logger.error("Skipping dose reminders for run with unparseable start_date")
                continue
            }

            var lines: [DoseReminderLine] = []
            // The run's own duration_days is the fallback if line detail
            // can't be loaded; it's replaced with the protocol's (more
            // authoritative) duration_days below on success.
            var durationDays = run.durationDays ?? Int.max
            do {
                let detail: ProtocolDetail = try await networkClient.request(
                    method: "GET",
                    path: Endpoints.protocolDetail(run.protocolId),
                    body: nil as String?
                )
                lines = detail.lines.map {
                    DoseReminderLine(substance: $0.substance, dose: $0.dose, unit: $0.unit, schedulePattern: $0.schedulePattern)
                }
                durationDays = detail.durationDays
            } catch {
                // Fall back to a daily, generically-worded reminder rather
                // than scheduling nothing — see DoseReminderScheduler.
                logger.notice("Could not load protocol detail for dose reminders; falling back to a daily reminder")
            }

            guard !Task.isCancelled else { return }

            doseRuns.append(
                DoseReminderRun(
                    runId: run.id,
                    protocolId: run.protocolId,
                    protocolName: run.protocolName ?? "your protocol",
                    startDate: startDate,
                    notify: run.notify,
                    notifyTimes: Self.notifyTimes(for: run),
                    lines: lines,
                    durationDays: durationDays
                )
            )
        }

        guard isAuthenticated(), !Task.isCancelled else { return }
        await notificationManager.scheduleDoseReminders(runs: doseRuns)
    }

    func clearAll() async {
        inFlightTask?.cancel()
        await notificationManager.clearAllDoseReminders()
    }

    // MARK: - Helpers

    private static func notifyTimes(for run: ActiveRunResponse) -> [String] {
        if let times = run.notifyTimes, !times.isEmpty {
            return times
        }
        if let single = run.notifyTime, !single.isEmpty {
            return [single]
        }
        return ["09:00"]
    }

    private static func parseDate(_ string: String) -> Date? {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.locale = Locale(identifier: "en_US_POSIX")
        return formatter.date(from: string)
    }
}
