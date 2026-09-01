// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("DoseReminderCoordinator", .serialized)
@MainActor
struct DoseReminderCoordinatorTests {
    private static func makeCoordinator(
        network: MockNetworkClient,
        notifications: MockNotificationManager,
        isAuthenticated: @escaping @MainActor () -> Bool = { true }
    ) -> DoseReminderCoordinator {
        DoseReminderCoordinator(
            networkClient: network,
            notificationManager: notifications,
            isAuthenticated: isAuthenticated
        )
    }

    private static func makeRun(
        id: String = "run-1",
        protocolId: String = "proto-1",
        notify: Bool = true,
        notifyTimes: [String]? = ["08:00"],
        durationDays: Int? = 30
    ) -> ActiveRunResponse {
        ActiveRunResponse(
            id: id,
            protocolId: protocolId,
            protocolName: "Test Protocol",
            // date-ok
            startDate: "2026-06-01",
            durationDays: durationDays,
            status: "active",
            notify: notify,
            notifyTime: nil,
            notifyTimes: notifyTimes,
            repeatReminders: false,
            repeatIntervalMinutes: nil,
            progressPct: 10,
            dosesToday: 1,
            dosesCompletedToday: 0,
            // date-ok
            createdAt: "2026-06-01T00:00:00Z"
        )
    }

    private static func makeDetail(lines: [ProtocolLine], durationDays: Int = 30) -> ProtocolDetail {
        ProtocolDetail(
            id: "proto-1",
            userId: "user-1",
            name: "Test Protocol",
            description: nil,
            status: .active,
            // date-ok
            startDate: "2026-06-01",
            durationDays: durationDays,
            shareToken: nil,
            // date-ok
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
        let detail = Self.makeDetail(lines: [Self.makeLine()], durationDays: 21)
        network.requestHandler = { _, path, _ in
            if path == Endpoints.activeRuns {
                return [Self.makeRun()]
            }
            return detail
        }
        let notifications = MockNotificationManager()
        let coordinator = Self.makeCoordinator(network: network, notifications: notifications)

        await coordinator.rebuildReminders()

        #expect(notifications.scheduleDoseRemindersCalls.count == 1)
        let scheduled = try #require(notifications.scheduleDoseRemindersCalls.first)
        let run = try #require(scheduled.first)
        #expect(run.runId == "run-1")
        #expect(run.lines.map(\.substance) == ["Creatine"])
        #expect(run.notifyTimes == ["08:00"])
        // Protocol detail's duration_days is authoritative when available.
        #expect(run.durationDays == 21)
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
        let coordinator = Self.makeCoordinator(network: network, notifications: notifications)

        await coordinator.rebuildReminders()

        let scheduled = try #require(notifications.scheduleDoseRemindersCalls.first)
        #expect(scheduled.isEmpty)
    }

    @Test("rebuildReminders falls back to an empty line set and the run's own duration_days when protocol detail fetch fails")
    func rebuildFallsBackWhenDetailFetchFails() async throws {
        let network = MockNetworkClient()
        network.requestHandler = { _, path, _ in
            if path == Endpoints.activeRuns {
                return [Self.makeRun(durationDays: 14)]
            }
            throw NetworkError.serverError(statusCode: 500, body: "boom")
        }
        let notifications = MockNotificationManager()
        let coordinator = Self.makeCoordinator(network: network, notifications: notifications)

        await coordinator.rebuildReminders()

        let scheduled = try #require(notifications.scheduleDoseRemindersCalls.first)
        let run = try #require(scheduled.first)
        #expect(run.lines.isEmpty) // triggers the daily fallback in DoseReminderScheduler
        #expect(run.durationDays == 14) // falls back to the run's own duration_days
    }

    @Test("rebuildReminders resolves notifyTime when notifyTimes is absent")
    func rebuildFallsBackToSingleNotifyTime() async throws {
        let network = MockNetworkClient()
        let run = ActiveRunResponse(
            id: "run-1",
            protocolId: "proto-1",
            protocolName: "Test",
            // date-ok
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
            // date-ok
            createdAt: "2026-06-01T00:00:00Z"
        )
        network.requestHandler = { _, path, _ in
            if path == Endpoints.activeRuns { return [run] }
            return Self.makeDetail(lines: [])
        }
        let notifications = MockNotificationManager()
        let coordinator = Self.makeCoordinator(network: network, notifications: notifications)

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
        let coordinator = Self.makeCoordinator(network: network, notifications: notifications)

        await coordinator.rebuildReminders()

        #expect(notifications.scheduleDoseRemindersCalls.isEmpty)
    }

    @Test("rebuildReminders with no active runs schedules an empty set (clearing stale reminders)")
    func rebuildWithNoActiveRunsSchedulesEmpty() async throws {
        let network = MockNetworkClient()
        network.requestHandler = { _, _, _ in [ActiveRunResponse]() }
        let notifications = MockNotificationManager()
        let coordinator = Self.makeCoordinator(network: network, notifications: notifications)

        await coordinator.rebuildReminders()

        let scheduled = try #require(notifications.scheduleDoseRemindersCalls.first)
        #expect(scheduled.isEmpty)
    }

    // MARK: - Logout race

    @Test("rebuildReminders does not schedule anything if auth is revoked while the active-runs fetch is in flight")
    func rebuildBailsWhenAuthRevokedDuringInitialFetch() async {
        let network = MockNetworkClient()
        nonisolated(unsafe) var authenticated = true
        network.asyncRequestHandler = { _, path, _ in
            if path == Endpoints.activeRuns {
                // Simulate a logout completing while this request is still
                // in flight — by the time it resolves, the user is signed out.
                authenticated = false
                let run = await Self.makeRun()
                return [run] as [ActiveRunResponse]
            }
            let line = await Self.makeLine()
            return await Self.makeDetail(lines: [line])
        }
        let notifications = MockNotificationManager()
        let coordinator = Self.makeCoordinator(
            network: network,
            notifications: notifications,
            isAuthenticated: { authenticated }
        )

        await coordinator.rebuildReminders()

        #expect(notifications.scheduleDoseRemindersCalls.isEmpty)
    }

    @Test("rebuildReminders does not schedule anything if auth is revoked while fetching protocol detail")
    func rebuildBailsWhenAuthRevokedDuringDetailFetch() async {
        let network = MockNetworkClient()
        nonisolated(unsafe) var authenticated = true
        network.asyncRequestHandler = { _, path, _ in
            if path == Endpoints.activeRuns {
                let run = await Self.makeRun()
                return [run] as [ActiveRunResponse]
            }
            // Logout lands while resolving the per-run protocol detail fetch.
            authenticated = false
            let line = await Self.makeLine()
            return await Self.makeDetail(lines: [line])
        }
        let notifications = MockNotificationManager()
        let coordinator = Self.makeCoordinator(
            network: network,
            notifications: notifications,
            isAuthenticated: { authenticated }
        )

        await coordinator.rebuildReminders()

        #expect(notifications.scheduleDoseRemindersCalls.isEmpty)
    }

    @Test("rebuildReminders does not run at all when already signed out")
    func rebuildNoOpsWhenNotAuthenticated() async {
        let network = MockNetworkClient()
        network.requestHandler = { _, _, _ in
            Issue.record("Should not make any network calls when not authenticated")
            return [ActiveRunResponse]()
        }
        let notifications = MockNotificationManager()
        let coordinator = Self.makeCoordinator(network: network, notifications: notifications, isAuthenticated: { false })

        await coordinator.rebuildReminders()

        #expect(notifications.scheduleDoseRemindersCalls.isEmpty)
    }

    // MARK: - Coalescing

    @Test("a rebuild superseded by a newer, concurrent call does not schedule its (stale) result")
    func overlappingRebuildsCoalesceToLatestOnly() async {
        let network = MockNetworkClient()
        nonisolated(unsafe) var callIndex = 0
        let gate = ContinuationGate()
        network.asyncRequestHandler = { _, path, _ in
            guard path == Endpoints.activeRuns else {
                return await Self.makeDetail(lines: [])
            }
            callIndex += 1
            let index = callIndex
            if index == 1 {
                // Pause the first call's fetch until the second call has
                // started and cancelled it.
                await gate.wait()
            }
            let run = await Self.makeRun(id: "run-\(index)")
            return [run] as [ActiveRunResponse]
        }
        let notifications = MockNotificationManager()
        let coordinator = Self.makeCoordinator(network: network, notifications: notifications)

        let firstTask = Task { await coordinator.rebuildReminders() }
        try? await Task.sleep(nanoseconds: 20_000_000) // let the first call reach the gate
        let secondTask = Task { await coordinator.rebuildReminders() }
        try? await Task.sleep(nanoseconds: 20_000_000) // let the second call start and cancel the first
        await gate.open() // release the first call's paused fetch

        _ = await (firstTask.value, secondTask.value)

        // The superseded first call is cancelled after its fetch resumes and
        // must not schedule anything; only the second call's data should
        // ever reach the notification manager.
        #expect(notifications.scheduleDoseRemindersCalls.count == 1)
        #expect(notifications.scheduleDoseRemindersCalls.first?.first?.runId == "run-2")
    }

    // MARK: - clearAll

    @Test("clearAll delegates to the notification manager")
    func clearAllDelegates() async {
        let network = MockNetworkClient()
        let notifications = MockNotificationManager()
        let coordinator = Self.makeCoordinator(network: network, notifications: notifications)

        await coordinator.clearAll()

        #expect(notifications.clearAllDoseRemindersCallCount == 1)
    }
}

/// A one-shot async gate used to deterministically pause and later release
/// a mock network handler mid-fetch, so concurrency tests don't rely on
/// sleep-based timing to prove ordering.
private final class ContinuationGate: @unchecked Sendable {
    private var continuation: CheckedContinuation<Void, Never>?

    func wait() async {
        await withCheckedContinuation { self.continuation = $0 }
    }

    func open() async {
        continuation?.resume()
        continuation = nil
    }
}
