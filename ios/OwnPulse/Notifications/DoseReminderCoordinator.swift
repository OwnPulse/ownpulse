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

    init(networkClient: NetworkClientProtocol, notificationManager: NotificationManagerProtocol) {
        self.networkClient = networkClient
        self.notificationManager = notificationManager
    }

    func rebuildReminders() async {
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

        let notifyRuns = runs.filter(\.notify)
        var doseRuns: [DoseReminderRun] = []
        doseRuns.reserveCapacity(notifyRuns.count)

        for run in notifyRuns {
            guard let startDate = Self.parseDate(run.startDate) else {
                logger.error("Skipping dose reminders for run with unparseable start_date")
                continue
            }

            var lines: [DoseReminderLine] = []
            do {
                let detail: ProtocolDetail = try await networkClient.request(
                    method: "GET",
                    path: Endpoints.protocolDetail(run.protocolId),
                    body: nil as String?
                )
                lines = detail.lines.map {
                    DoseReminderLine(substance: $0.substance, dose: $0.dose, unit: $0.unit, schedulePattern: $0.schedulePattern)
                }
            } catch {
                // Fall back to a daily, generically-worded reminder rather
                // than scheduling nothing — see DoseReminderScheduler.
                logger.notice("Could not load protocol detail for dose reminders; falling back to a daily reminder")
            }

            doseRuns.append(
                DoseReminderRun(
                    runId: run.id,
                    protocolId: run.protocolId,
                    protocolName: run.protocolName ?? "your protocol",
                    startDate: startDate,
                    notify: run.notify,
                    notifyTimes: Self.notifyTimes(for: run),
                    lines: lines
                )
            )
        }

        await notificationManager.scheduleDoseReminders(runs: doseRuns)
    }

    func clearAll() async {
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
