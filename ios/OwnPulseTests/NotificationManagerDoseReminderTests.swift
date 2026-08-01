// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
import UserNotifications
@testable import OwnPulse

@Suite("NotificationManager — dose reminders", .serialized)
@MainActor
struct NotificationManagerDoseReminderTests {
    // `NotificationManager.scheduleDoseReminders` defers to
    // `DoseReminderScheduler.computeSpecs` using `Calendar.current` (the
    // device's local calendar) — it doesn't expose a calendar override, by
    // design, since production always wants the user's local time. These
    // tests build dates with the same `Calendar.current` so day-boundary
    // math (and the identifiers it produces) is internally consistent
    // regardless of which timezone the test host is in.
    private func date(_ string: String, hour: Int = 0, minute: Int = 0) -> Date {
        let calendar = Calendar.current
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.locale = Locale(identifier: "en_US_POSIX")
        var components = calendar.dateComponents([.year, .month, .day], from: formatter.date(from: string)!)
        components.hour = hour
        components.minute = minute
        return calendar.date(from: components)!
    }

    private func makeRun(runId: String = "run-1", notify: Bool = true) -> DoseReminderRun {
        DoseReminderRun(
            runId: runId,
            protocolId: "proto-1",
            protocolName: "Test Protocol",
            startDate: date("2026-06-01"),
            notify: notify,
            notifyTimes: ["09:00"],
            lines: []
        )
    }

    // MARK: - scheduleDoseReminders — success

    @Test("scheduleDoseReminders adds one request per computed spec")
    func scheduleAddsRequests() async {
        let center = MockUserNotificationCenter()
        let manager = NotificationManager(networkClient: MockNetworkClient(), notificationCenter: center)

        await manager.scheduleDoseReminders(runs: [makeRun()], now: date("2026-06-01"))

        #expect(center.addedRequests.count == 7)
        #expect(center.addedRequests.allSatisfy { $0.identifier.hasPrefix("dose-run-1-") })
    }

    @Test("scheduleDoseReminders removes stale dose reminders no longer in the computed set")
    func scheduleRemovesStaleReminders() async {
        let center = MockUserNotificationCenter()
        let staleContent = UNMutableNotificationContent()
        let staleRequest = UNNotificationRequest(
            identifier: "dose-run-1-09:00-2099-01-01",
            content: staleContent,
            trigger: nil
        )
        center.pendingRequests = [staleRequest]

        let manager = NotificationManager(networkClient: MockNetworkClient(), notificationCenter: center)
        await manager.scheduleDoseReminders(runs: [makeRun()], now: date("2026-06-01"))

        #expect(center.removedIdentifierBatches.flatMap { $0 }.contains("dose-run-1-09:00-2099-01-01"))
    }

    @Test("scheduleDoseReminders never removes non-dose-reminder pending requests")
    func scheduleLeavesUnrelatedRequestsAlone() async {
        let center = MockUserNotificationCenter()
        let otherRequest = UNNotificationRequest(
            identifier: "some-other-notification",
            content: UNMutableNotificationContent(),
            trigger: nil
        )
        center.pendingRequests = [otherRequest]

        let manager = NotificationManager(networkClient: MockNetworkClient(), notificationCenter: center)
        await manager.scheduleDoseReminders(runs: [makeRun()], now: date("2026-06-01"))

        let removedIds = center.removedIdentifierBatches.flatMap { $0 }
        #expect(!removedIds.contains("some-other-notification"))
    }

    @Test("scheduleDoseReminders with notify=false schedules nothing")
    func scheduleWithNotifyFalseSchedulesNothing() async {
        let center = MockUserNotificationCenter()
        let manager = NotificationManager(networkClient: MockNetworkClient(), notificationCenter: center)

        await manager.scheduleDoseReminders(runs: [makeRun(notify: false)], now: date("2026-06-01"))

        #expect(center.addedRequests.isEmpty)
    }

    // MARK: - scheduleDoseReminders — failure path

    @Test("scheduleDoseReminders continues past a failed add() without throwing")
    func scheduleToleratesAddFailure() async {
        let center = MockUserNotificationCenter()
        center.addError = NSError(domain: "test", code: 1)
        let manager = NotificationManager(networkClient: MockNetworkClient(), notificationCenter: center)

        // Should not throw/crash — errors are logged and swallowed per-request.
        await manager.scheduleDoseReminders(runs: [makeRun()], now: date("2026-06-01"))

        #expect(center.addedRequests.isEmpty)
    }

    @Test("scheduleDoseReminders truncates past the 64-pending cap without adding more than 64")
    func scheduleRespectsCap() async {
        let center = MockUserNotificationCenter()
        let manager = NotificationManager(networkClient: MockNetworkClient(), notificationCenter: center)

        let runs = (0..<10).map { i in
            DoseReminderRun(
                runId: "run-\(i)",
                protocolId: "proto-\(i)",
                protocolName: "Protocol \(i)",
                startDate: date("2026-06-01"),
                notify: true,
                notifyTimes: ["08:00", "20:00"],
                lines: []
            )
        }

        await manager.scheduleDoseReminders(runs: runs, now: date("2026-06-01"))

        #expect(center.addedRequests.count == 64)
    }

    // MARK: - clearDoseReminders

    @Test("clearDoseReminders removes only requests for the given run")
    func clearDoseRemindersRemovesMatchingRun() async {
        let center = MockUserNotificationCenter()
        center.pendingRequests = [
            UNNotificationRequest(identifier: "dose-run-1-09:00-2026-06-01", content: UNMutableNotificationContent(), trigger: nil),
            UNNotificationRequest(identifier: "dose-run-2-09:00-2026-06-01", content: UNMutableNotificationContent(), trigger: nil),
        ]
        let manager = NotificationManager(networkClient: MockNetworkClient(), notificationCenter: center)

        await manager.clearDoseReminders(runId: "run-1")

        let removedIds = center.removedIdentifierBatches.flatMap { $0 }
        #expect(removedIds.contains("dose-run-1-09:00-2026-06-01"))
        #expect(!removedIds.contains("dose-run-2-09:00-2026-06-01"))
    }

    @Test("clearDoseReminders is a no-op when there is nothing pending for the run")
    func clearDoseRemindersNoOpWhenEmpty() async {
        let center = MockUserNotificationCenter()
        let manager = NotificationManager(networkClient: MockNetworkClient(), notificationCenter: center)

        await manager.clearDoseReminders(runId: "run-1")

        #expect(center.removedIdentifierBatches.isEmpty)
    }

    // MARK: - clearAllDoseReminders

    @Test("clearAllDoseReminders removes every dose reminder but leaves other notifications")
    func clearAllRemovesOnlyDoseReminders() async {
        let center = MockUserNotificationCenter()
        center.pendingRequests = [
            UNNotificationRequest(identifier: "dose-run-1-09:00-2026-06-01", content: UNMutableNotificationContent(), trigger: nil),
            UNNotificationRequest(identifier: "dose-run-2-20:00-2026-06-02", content: UNMutableNotificationContent(), trigger: nil),
            UNNotificationRequest(identifier: "unrelated", content: UNMutableNotificationContent(), trigger: nil),
        ]
        let manager = NotificationManager(networkClient: MockNetworkClient(), notificationCenter: center)

        await manager.clearAllDoseReminders()

        let removedIds = Set(center.removedIdentifierBatches.flatMap { $0 })
        #expect(removedIds.contains("dose-run-1-09:00-2026-06-01"))
        #expect(removedIds.contains("dose-run-2-20:00-2026-06-02"))
        #expect(!removedIds.contains("unrelated"))
    }
}
