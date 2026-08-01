// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("DoseReminderCoordinator", .serialized)
@MainActor
struct DoseReminderCoordinatorTests {
    private static func makeRun(
        id: String = "run-1",
        protocolId: String = "proto-1",
        notify: Bool = true,
        notifyTimes: [String]? = ["08:00"]
    ) -> ActiveRunResponse {
        ActiveRunResponse(
            id: id,
            protocolId: protocolId,
            protocolName: "Test Protocol",
            startDate: "2026-06-01",
            durationDays: 30,
            status: "active",
            notify: notify,
            notifyTime: nil,
            notifyTimes: notifyTimes,
            repeatReminders: false,
            repeatIntervalMinutes: nil,
            progressPct: 10,
            dosesToday: 1,
            dosesCompletedToday: 0,
            createdAt: "2026-06-01T00:00:00Z"
        )
    }

    private static func makeDetail(lines: [ProtocolLine]) -> ProtocolDetail {
        ProtocolDetail(
            id: "proto-1",
            userId: "user-1",
            name: "Test Protocol",
            description: nil,
            status: .active,
            startDate: "2026-06-01",
            durationDays: 30,
            shareToken: nil,
            createdAt: "2026-06-01T00:00:00Z",
            lines: lines
        )
    }

    private static func makeLine(substance: String = "Creatine") -> ProtocolLine {
        ProtocolLine(
            id: "line-1",
            protocolId: "proto-1",
            substance: substance,
            dose: 5,
            unit: "g",
            route: nil,
            timeOfDay: nil,
            schedulePattern: Array(repeating: true, count: 30),
            sortOrder: 0,
            doses: []
        )
    }

    // MARK: - Success path

    @Test("rebuildReminders fetches active runs, resolves line detail, and schedules reminders")
    func rebuildSchedulesForNotifyEnabledRuns() async throws {
        let network = MockNetworkClient()
        let detail = Self.makeDetail(lines: [Self.makeLine()])
        network.requestHandler = { _, path, _ in
            if path == Endpoints.activeRuns {
                return [Self.makeRun()]
            }
            return detail
        }
        let notifications = MockNotificationManager()
        let coordinator = DoseReminderCoordinator(networkClient: network, notificationManager: notifications)

        await coordinator.rebuildReminders()

        #expect(notifications.scheduleDoseRemindersCalls.count == 1)
        let scheduled = try #require(notifications.scheduleDoseRemindersCalls.first)
        let run = try #require(scheduled.first)
        #expect(run.runId == "run-1")
        #expect(run.lines.map(\.substance) == ["Creatine"])
        #expect(run.notifyTimes == ["08:00"])
    }

    @Test("rebuildReminders excludes runs with notify disabled from the scheduled set")
    func rebuildExcludesNotifyDisabledRuns() async throws {
        let network = MockNetworkClient()
        network.requestHandler = { _, path, _ in
            if path == Endpoints.activeRuns {
                return [Self.makeRun(notify: false)]
            }
            Issue.record("Should not fetch protocol detail for a notify-disabled run")
            return Self.makeDetail(lines: [])
        }
        let notifications = MockNotificationManager()
        let coordinator = DoseReminderCoordinator(networkClient: network, notificationManager: notifications)

        await coordinator.rebuildReminders()

        let scheduled = try #require(notifications.scheduleDoseRemindersCalls.first)
        #expect(scheduled.isEmpty)
    }

    @Test("rebuildReminders falls back to an empty line set when protocol detail fetch fails")
    func rebuildFallsBackWhenDetailFetchFails() async throws {
        let network = MockNetworkClient()
        network.requestHandler = { _, path, _ in
            if path == Endpoints.activeRuns {
                return [Self.makeRun()]
            }
            throw NetworkError.serverError(statusCode: 500, body: "boom")
        }
        let notifications = MockNotificationManager()
        let coordinator = DoseReminderCoordinator(networkClient: network, notificationManager: notifications)

        await coordinator.rebuildReminders()

        let scheduled = try #require(notifications.scheduleDoseRemindersCalls.first)
        let run = try #require(scheduled.first)
        #expect(run.lines.isEmpty) // triggers the daily fallback in DoseReminderScheduler
    }

    @Test("rebuildReminders resolves notifyTime when notifyTimes is absent")
    func rebuildFallsBackToSingleNotifyTime() async throws {
        let network = MockNetworkClient()
        let run = ActiveRunResponse(
            id: "run-1",
            protocolId: "proto-1",
            protocolName: "Test",
            startDate: "2026-06-01",
            durationDays: 30,
            status: "active",
            notify: true,
            notifyTime: "07:30",
            notifyTimes: nil,
            repeatReminders: false,
            repeatIntervalMinutes: nil,
            progressPct: 0,
            dosesToday: 0,
            dosesCompletedToday: 0,
            createdAt: "2026-06-01T00:00:00Z"
        )
        network.requestHandler = { _, path, _ in
            if path == Endpoints.activeRuns { return [run] }
            return Self.makeDetail(lines: [])
        }
        let notifications = MockNotificationManager()
        let coordinator = DoseReminderCoordinator(networkClient: network, notificationManager: notifications)

        await coordinator.rebuildReminders()

        let scheduled = try #require(notifications.scheduleDoseRemindersCalls.first?.first)
        #expect(scheduled.notifyTimes == ["07:30"])
    }

    // MARK: - Error path

    @Test("rebuildReminders leaves existing reminders untouched when fetching active runs fails")
    func rebuildDoesNotScheduleOnNetworkFailure() async {
        let network = MockNetworkClient()
        network.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "boom")
        }
        let notifications = MockNotificationManager()
        let coordinator = DoseReminderCoordinator(networkClient: network, notificationManager: notifications)

        await coordinator.rebuildReminders()

        #expect(notifications.scheduleDoseRemindersCalls.isEmpty)
    }

    @Test("rebuildReminders with no active runs schedules an empty set (clearing stale reminders)")
    func rebuildWithNoActiveRunsSchedulesEmpty() async throws {
        let network = MockNetworkClient()
        network.requestHandler = { _, _, _ in [ActiveRunResponse]() }
        let notifications = MockNotificationManager()
        let coordinator = DoseReminderCoordinator(networkClient: network, notificationManager: notifications)

        await coordinator.rebuildReminders()

        let scheduled = try #require(notifications.scheduleDoseRemindersCalls.first)
        #expect(scheduled.isEmpty)
    }

    // MARK: - clearAll

    @Test("clearAll delegates to the notification manager")
    func clearAllDelegates() async {
        let network = MockNetworkClient()
        let notifications = MockNotificationManager()
        let coordinator = DoseReminderCoordinator(networkClient: network, notificationManager: notifications)

        await coordinator.clearAll()

        #expect(notifications.clearAllDoseRemindersCallCount == 1)
    }
}
