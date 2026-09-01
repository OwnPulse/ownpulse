// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("ProtocolsViewModel", .serialized)
@MainActor
struct ProtocolsViewModelTests {
    // MARK: - Test Fixtures

    private static func makeListItem(
        id: String = "proto-1",
        name: String = "Test Protocol",
        status: ProtocolStatus = .active,
        durationDays: Int = 28,
        progressPct: Double = 0
    ) -> ProtocolListItem {
        ProtocolListItem(
            id: id,
            name: name,
            status: status,
            // date-ok
            startDate: "2026-03-01",
            durationDays: durationDays,
            isTemplate: false,
            progressPct: progressPct,
            nextDose: nil,
            // date-ok
            createdAt: "2026-03-01T00:00:00Z"
        )
    }

    private static func makeActiveRun(
        id: String = "run-1",
        protocolId: String = "proto-1",
        protocolName: String = "Test Protocol",
        progressPct: Double = 18.0,
        dosesToday: Int = 2,
        dosesCompletedToday: Int = 0,
        notify: Bool = false,
        // date-ok
        createdAt: String = "2026-03-28T10:00:00Z"
    ) -> ActiveRunResponse {
        ActiveRunResponse(
            id: id,
            protocolId: protocolId,
            protocolName: protocolName,
            // date-ok
            startDate: "2026-03-28",
            durationDays: 28,
            status: "active",
            notify: notify,
            notifyTime: nil,
            notifyTimes: nil,
            repeatReminders: false,
            repeatIntervalMinutes: nil,
            progressPct: progressPct,
            dosesToday: dosesToday,
            dosesCompletedToday: dosesCompletedToday,
            createdAt: createdAt
        )
    }

    private static func makeDetail(
        id: String = "proto-1",
        name: String = "Test Protocol",
        status: ProtocolStatus = .active,
        durationDays: Int = 28,
        lines: [ProtocolLine] = [],
        runs: [ActiveRunResponse]? = nil
    ) -> ProtocolDetail {
        ProtocolDetail(
            id: id,
            userId: "user-1",
            name: name,
            description: "Test description",
            status: status,
            // date-ok
            startDate: "2026-03-01",
            durationDays: durationDays,
            shareToken: nil,
            // date-ok
            createdAt: "2026-03-01T00:00:00Z",
            lines: lines,
            runs: runs
        )
    }

    private static func makeLine(
        id: String = "line-1",
        substance: String = "BPC-157",
        dose: Double? = 250,
        unit: String? = "mcg",
        route: String? = "SubQ",
        durationDays: Int = 28,
        allOn: Bool = true,
        doses: [ProtocolDose] = []
    ) -> ProtocolLine {
        ProtocolLine(
            id: id,
            protocolId: "proto-1",
            substance: substance,
            dose: dose,
            unit: unit,
            route: route,
            timeOfDay: nil,
            schedulePattern: Array(repeating: allOn, count: durationDays),
            sortOrder: 0,
            doses: doses
        )
    }

    private static func makeDose(
        id: String = "dose-1",
        lineId: String = "line-1",
        dayNumber: Int = 0,
        status: DoseStatus = .completed
    ) -> ProtocolDose {
        ProtocolDose(
            id: id,
            protocolLineId: lineId,
            dayNumber: dayNumber,
            status: status,
            interventionId: nil,
            // date-ok
            loggedAt: "2026-03-01T08:00:00Z",
            runId: nil,
            skipReason: nil
        )
    }

    // MARK: - Load Protocols - Success

    @Test("loadProtocols success transitions idle -> loading -> loaded")
    func loadProtocolsSuccess() async {
        let mock = MockNetworkClient()
        let items = [
            Self.makeListItem(id: "p1", name: "Protocol A"),
            Self.makeListItem(id: "p2", name: "Protocol B", status: .completed),
        ]
        let runs = [Self.makeActiveRun()]
        mock.requestHandler = { _, path, _ in
            if path == Endpoints.activeRuns {
                return runs
            }
            return items
        }

        let vm = ProtocolsViewModel(networkClient: mock)
        #expect(vm.listState == .idle)

        await vm.loadProtocols()

        #expect(vm.listState == .loaded)
        #expect(vm.protocols.count == 2)
        #expect(vm.activeRuns.count == 1)
        #expect(mock.requestCalls.count == 2)
    }

    // MARK: - Load Protocols - Error

    @Test("loadProtocols failure transitions to error state")
    func loadProtocolsFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "internal error")
        }

        let vm = ProtocolsViewModel(networkClient: mock)

        await vm.loadProtocols()

        if case .error(let msg) = vm.listState {
            #expect(msg == "Failed to load protocols")
        } else {
            Issue.record("Expected error state")
        }
    }

    // MARK: - Load Protocols - Unauthorized

    @Test("loadProtocols unauthorized transitions to error state")
    func loadProtocolsUnauthorized() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.unauthorized
        }

        let vm = ProtocolsViewModel(networkClient: mock)

        await vm.loadProtocols()

        if case .error = vm.listState {
            // expected
        } else {
            Issue.record("Expected error state for unauthorized")
        }
    }

    // MARK: - Filtering

    @Test("filteredProtocols filters by active status")
    func filterActive() async {
        let mock = MockNetworkClient()
        let items = [
            Self.makeListItem(id: "p1", status: .active),
            Self.makeListItem(id: "p2", status: .paused),
            Self.makeListItem(id: "p3", status: .completed),
        ]
        let runs: [ActiveRunResponse] = []
        mock.requestHandler = { _, path, _ in
            if path == Endpoints.activeRuns { return runs }
            return items
        }

        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.loadProtocols()

        vm.filter = .active
        #expect(vm.filteredProtocols.count == 2)

        vm.filter = .completed
        #expect(vm.filteredProtocols.count == 1)

        vm.filter = .all
        #expect(vm.filteredProtocols.count == 3)
    }

    // MARK: - Load Detail - Success

    @Test("loadProtocol success loads detail")
    func loadDetailSuccess() async {
        let mock = MockNetworkClient()
        let detail = Self.makeDetail()
        mock.requestHandler = { _, _, _ in detail }

        let vm = ProtocolsViewModel(networkClient: mock)
        #expect(vm.detailState == .idle)

        await vm.loadProtocol(id: "proto-1")

        #expect(vm.detailState == .loaded)
        #expect(vm.selectedProtocol?.id == "proto-1")
        #expect(mock.requestCalls[0].path == Endpoints.protocolDetail("proto-1"))
    }

    // MARK: - Load Detail - Error

    @Test("loadProtocol failure transitions to error state")
    func loadDetailFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 404, body: "not found")
        }

        let vm = ProtocolsViewModel(networkClient: mock)

        await vm.loadProtocol(id: "nonexistent")

        if case .error(let msg) = vm.detailState {
            #expect(msg == "Failed to load protocol")
        } else {
            Issue.record("Expected error state")
        }
    }

    // MARK: - Create Protocol - Success

    @Test("createProtocol success transitions to success state and resets form")
    func createProtocolSuccess() async {
        let mock = MockNetworkClient()
        let detail = Self.makeDetail()
        mock.requestHandler = { _, _, _ in detail }

        let vm = ProtocolsViewModel(networkClient: mock)
        vm.newName = "My Protocol"
        vm.newLines[0].substance = "BPC-157"
        vm.newLines[0].dose = "250"
        vm.newLines[0].unit = "mcg"

        await vm.createProtocol()

        #expect(vm.createState == .success("Protocol created"))
        // Verify form was reset
        #expect(vm.newName == "")
        #expect(vm.newLines.count == 1)
        #expect(vm.newLines[0].substance == "")
        // Verify network call
        #expect(mock.requestCalls.count == 1)
        #expect(mock.requestCalls[0].method == "POST")
        #expect(mock.requestCalls[0].path == Endpoints.protocols)
    }

    // MARK: - Create Protocol - Error

    @Test("createProtocol failure transitions to error state")
    func createProtocolFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 422, body: "validation failed")
        }

        let vm = ProtocolsViewModel(networkClient: mock)
        vm.newName = "My Protocol"
        vm.newLines[0].substance = "BPC-157"

        await vm.createProtocol()

        if case .error(let msg) = vm.createState {
            #expect(msg.contains("Failed to create protocol"))
        } else {
            Issue.record("Expected error state")
        }
    }

    // MARK: - Create Protocol - Validation

    @Test("createProtocol with empty name shows validation error")
    func createProtocolValidationEmptyName() async {
        let mock = MockNetworkClient()
        let vm = ProtocolsViewModel(networkClient: mock)
        vm.newLines[0].substance = "BPC-157"
        // name is empty

        await vm.createProtocol()

        #expect(vm.createState == .error("Name and at least one substance are required"))
        #expect(mock.requestCalls.isEmpty)
    }

    @Test("createProtocol with empty substance shows validation error")
    func createProtocolValidationEmptySubstance() async {
        let mock = MockNetworkClient()
        let vm = ProtocolsViewModel(networkClient: mock)
        vm.newName = "My Protocol"
        // substance is empty

        await vm.createProtocol()

        #expect(vm.createState == .error("Name and at least one substance are required"))
        #expect(mock.requestCalls.isEmpty)
    }

    @Test("createProtocol with whitespace-only name shows validation error")
    func createProtocolValidationWhitespaceName() async {
        let mock = MockNetworkClient()
        let vm = ProtocolsViewModel(networkClient: mock)
        vm.newName = "   "
        vm.newLines[0].substance = "BPC-157"

        await vm.createProtocol()

        #expect(vm.createState == .error("Name and at least one substance are required"))
        #expect(mock.requestCalls.isEmpty)
    }

    // MARK: - createIsValid

    @Test("createIsValid reflects name and substance state")
    func createIsValid() {
        let mock = MockNetworkClient()
        let vm = ProtocolsViewModel(networkClient: mock)

        #expect(vm.createIsValid == false) // empty name and substance

        vm.newName = "Protocol"
        #expect(vm.createIsValid == false) // empty substance

        vm.newLines[0].substance = "BPC-157"
        #expect(vm.createIsValid == true)

        vm.newName = "  "
        #expect(vm.createIsValid == false) // whitespace name
    }

    // MARK: - Line Management

    @Test("addLine and removeLine manage lines correctly")
    func lineManagement() {
        let mock = MockNetworkClient()
        let vm = ProtocolsViewModel(networkClient: mock)

        #expect(vm.newLines.count == 1)

        vm.addLine()
        #expect(vm.newLines.count == 2)

        vm.addLine()
        #expect(vm.newLines.count == 3)

        vm.removeLine(at: 1)
        #expect(vm.newLines.count == 2)

        // Cannot remove last line
        vm.removeLine(at: 0)
        vm.removeLine(at: 0)
        #expect(vm.newLines.count == 1)
    }

    // MARK: - Delete Protocol - Success

    @Test("deleteProtocol success returns true")
    func deleteProtocolSuccess() async {
        let mock = MockNetworkClient()
        mock.requestNoContentHandler = { _, _, _ in }

        let vm = ProtocolsViewModel(networkClient: mock)
        let result = await vm.deleteProtocol(id: "proto-1")

        #expect(result == true)
        #expect(mock.requestCalls.count == 1)
        #expect(mock.requestCalls[0].method == "DELETE")
        #expect(mock.requestCalls[0].path == Endpoints.protocolDetail("proto-1"))
    }

    // MARK: - Delete Protocol - Error

    @Test("deleteProtocol failure returns false")
    func deleteProtocolFailure() async {
        let mock = MockNetworkClient()
        mock.requestNoContentHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 404, body: "not found")
        }

        let vm = ProtocolsViewModel(networkClient: mock)
        let result = await vm.deleteProtocol(id: "nonexistent")

        #expect(result == false)
    }

    // MARK: - Dose Action Refresh Fixtures
    //
    // logDose/skipDose/undoDose now refresh adherence + run-doses +
    // missed-doses (in addition to the protocol detail) whenever a runId is
    // used — see `ProtocolsViewModel.refreshAfterDoseAction`. A
    // requestHandler that only branches on "doses/log" and falls back to a
    // ProtocolDetail for everything else crashes (`MockNetworkClient`'s
    // forced cast) the moment one of those three additional GETs runs. This
    // dispatcher returns the right fixture type for each path so dose-action
    // tests exercise the real refresh fan-out instead of avoiding it.
    private static func doseActionHandler(
        logDoseResponse: ProtocolDose = Self.makeDose(),
        protocolDetail: ProtocolDetail = Self.makeDetail(),
        adherence: AdherenceResponse = AdherenceResponse(
            runId: "run-1", scheduledSoFar: 0, completed: 0, skipped: 0, missed: 0, adherencePct: nil, lines: []
        ),
        runDoses: [RunDoseDay] = [],
        missedDoses: [MissedDoseItem] = []
    ) -> (String, String, (any Encodable & Sendable)?) throws -> Any {
        { method, path, _ in
            if method == "POST" && path.contains("doses/log") { return logDoseResponse }
            if path.contains("missed-doses") { return missedDoses }
            if path.contains("/adherence") { return adherence }
            if method == "GET" && path.contains("/doses") { return runDoses }
            return protocolDetail
        }
    }

    // MARK: - Log Dose

    @Test("logDose with runId uses run endpoint and refreshes adherence/doses/missed")
    func logDoseWithRun() async {
        let mock = MockNetworkClient()
        mock.requestHandler = Self.doseActionHandler()

        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.logDose(protocolId: "proto-1", runId: "run-1", lineId: "line-1", dayNumber: 0)

        #expect(mock.requestCalls[0].method == "POST")
        #expect(mock.requestCalls[0].path == Endpoints.runLogDose("run-1"))
        // Log + adherence + run-doses + missed-doses + quiet protocol re-fetch.
        #expect(mock.requestCalls.count == 5)
        #expect(mock.requestCalls.dropFirst().allSatisfy { $0.method == "GET" })
    }

    @Test("logDose without runId uses legacy endpoint and skips the run-scoped refresh")
    func logDoseLegacy() async {
        let mock = MockNetworkClient()
        mock.requestHandler = Self.doseActionHandler()

        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.logDose(protocolId: "proto-1", runId: nil, lineId: "line-1", dayNumber: 0)

        // No runId — only the log call and the quiet protocol re-fetch, no
        // adherence/run-doses/missed-doses calls.
        #expect(mock.requestCalls.count == 2)
        #expect(mock.requestCalls[0].method == "POST")
        #expect(mock.requestCalls[0].path == Endpoints.protocolLogDose("proto-1"))
    }

    // MARK: - Skip Dose

    @Test("skipDose with runId uses run endpoint and refreshes adherence/doses/missed")
    func skipDoseWithRun() async {
        let mock = MockNetworkClient()
        mock.requestNoContentHandler = { _, _, _ in }
        mock.requestHandler = Self.doseActionHandler()

        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.skipDose(protocolId: "proto-1", runId: "run-1", lineId: "line-1", dayNumber: 0)

        #expect(mock.requestCalls[0].method == "POST")
        #expect(mock.requestCalls[0].path == Endpoints.runSkipDose("run-1"))
        // Skip + adherence + run-doses + missed-doses + quiet protocol
        // re-fetch — same shared refreshAfterDoseAction() as logDose (see
        // logDoseWithRun above). This count drifted out of sync when
        // loadMissedDoses() was added to that helper; keep it matching.
        #expect(mock.requestCalls.count == 5)
    }

    // MARK: - Dose Reminder Rebuild Hook

    @Test("loadProtocols success rebuilds dose reminders via the injected coordinator")
    func loadProtocolsRebuildsDoseReminders() async {
        let mock = MockNetworkClient()
        let runs = [Self.makeActiveRun(notify: true)]
        mock.requestHandler = { _, path, _ in
            if path == Endpoints.activeRuns { return runs }
            return [Self.makeListItem()]
        }
        let rebuilder = MockDoseReminderRebuilder()

        let vm = ProtocolsViewModel(networkClient: mock, doseReminderRebuilder: rebuilder)
        await vm.loadProtocols()

        #expect(rebuilder.rebuildCallCount == 1)
    }

    @Test("loadProtocols failure does not rebuild dose reminders")
    func loadProtocolsFailureSkipsDoseReminderRebuild() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "internal error")
        }
        let rebuilder = MockDoseReminderRebuilder()

        let vm = ProtocolsViewModel(networkClient: mock, doseReminderRebuilder: rebuilder)
        await vm.loadProtocols()

        #expect(rebuilder.rebuildCallCount == 0)
    }

    @Test("loadProtocols works without a dose reminder rebuilder configured")
    func loadProtocolsWithoutRebuilderDoesNotCrash() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, path, _ in
            if path == Endpoints.activeRuns { return [ActiveRunResponse]() }
            return [ProtocolListItem]()
        }

        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.loadProtocols()

        #expect(vm.listState == .loaded)
    }

    // MARK: - Adherence

    @Test("loadAdherence success stores response")
    func loadAdherenceSuccess() async {
        let mock = MockNetworkClient()
        let response = AdherenceResponse(
            runId: "run-1",
            scheduledSoFar: 8,
            completed: 3,
            skipped: 2,
            missed: 3,
            adherencePct: 50.0,
            lines: []
        )
        mock.requestHandler = { _, _, _ in response }

        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.loadAdherence(runId: "run-1")

        #expect(vm.adherenceState == .loaded)
        #expect(vm.adherence?.adherencePct == 50.0)
        #expect(mock.requestCalls[0].path == Endpoints.runAdherence("run-1"))
    }

    @Test("loadAdherence failure transitions to error state")
    func loadAdherenceFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in throw NetworkError.serverError(statusCode: 404, body: "not found") }

        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.loadAdherence(runId: "run-1")

        if case .error = vm.adherenceState {
            // expected
        } else {
            Issue.record("Expected error state")
        }
    }

    @Test("loadAdherence unauthorized transitions to error state")
    func loadAdherenceUnauthorized() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in throw NetworkError.unauthorized }

        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.loadAdherence(runId: "run-1")

        if case .error = vm.adherenceState {
            // expected
        } else {
            Issue.record("Expected error state for unauthorized")
        }
    }

    // MARK: - Run Doses

    @Test("loadRunDoses success stores doses and builds query string")
    func loadRunDosesSuccess() async {
        let mock = MockNetworkClient()
        let days = [
            // date-ok
            RunDoseDay(
                dayNumber: 3, date: "2026-04-04", protocolLineId: "line-1", substance: "BPC-157",
                dose: 250.0, unit: "mcg", route: "subq", timeOfDay: "AM", status: .missed,
                doseId: nil, interventionId: nil, skipReason: nil, loggedAt: nil
            )
        ]
        mock.requestHandler = { _, _, _ in days }

        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.loadRunDoses(runId: "run-1", fromDay: 0, toDay: 5)

        #expect(vm.runDosesState == .loaded)
        #expect(vm.runDoses.count == 1)
        #expect(mock.requestCalls[0].path == "/api/v1/protocols/runs/run-1/doses?from_day=0&to_day=5")
    }

    @Test("loadRunDoses failure transitions to error state")
    func loadRunDosesFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in throw NetworkError.serverError(statusCode: 400, body: "bad range") }

        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.loadRunDoses(runId: "run-1")

        if case .error = vm.runDosesState {
            // expected
        } else {
            Issue.record("Expected error state")
        }
    }

    @Test("loadRunDoses clears stale doses when switching to a different run")
    func loadRunDosesClearsOnDifferentRun() async {
        let mock = MockNetworkClient()
        // date-ok
        let runOneDay = RunDoseDay(
            dayNumber: 0, date: "2026-04-01", protocolLineId: "line-1", substance: "Run One",
            dose: nil, unit: nil, route: nil, timeOfDay: nil, status: .missed,
            doseId: nil, interventionId: nil, skipReason: nil, loggedAt: nil
        )
        mock.requestHandler = { _, _, _ in [runOneDay] }
        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.loadRunDoses(runId: "run-1")
        #expect(vm.runDoses.map(\.substance) == ["Run One"])

        // Switching to a different run should clear the previous run's rows
        // immediately (asserted via the state captured mid-flight below)
        // rather than showing them under the new run's loading spinner.
        // `asyncRequestHandler` is `@Sendable`, so it runs outside this
        // test's (implicit MainActor) isolation — the flag needs
        // `nonisolated(unsafe)` to be mutated from it, and reading the
        // MainActor-isolated view model's `runDoses` needs an explicit
        // `await`.
        nonisolated(unsafe) var sawClearedDuringLoad = false
        mock.asyncRequestHandler = { _, _, _ in
            sawClearedDuringLoad = await vm.runDoses.isEmpty
            return [RunDoseDay]()
        }
        await vm.loadRunDoses(runId: "run-2")

        #expect(sawClearedDuringLoad == true)
    }

    @Test("loadRunDoses does not clear doses when refreshing the same run")
    func loadRunDosesKeepsDataOnSameRunRefresh() async {
        let mock = MockNetworkClient()
        // date-ok
        let day = RunDoseDay(
            dayNumber: 0, date: "2026-04-01", protocolLineId: "line-1", substance: "Creatine",
            dose: nil, unit: nil, route: nil, timeOfDay: nil, status: .missed,
            doseId: nil, interventionId: nil, skipReason: nil, loggedAt: nil
        )
        mock.requestHandler = { _, _, _ in [day] }
        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.loadRunDoses(runId: "run-1")

        nonisolated(unsafe) var sawDataDuringReload = false
        mock.asyncRequestHandler = { _, _, _ in
            sawDataDuringReload = await !vm.runDoses.isEmpty
            return [day]
        }
        await vm.loadRunDoses(runId: "run-1")

        #expect(sawDataDuringReload == true)
    }

    // MARK: - Current Run (paused-run adherence/grid fallback)

    @Test("currentRun prefers the active run when one exists")
    func currentRunPrefersActive() async {
        let mock = MockNetworkClient()
        let vm = ProtocolsViewModel(networkClient: mock)
        // date-ok
        let active = Self.makeActiveRun(id: "run-active", createdAt: "2026-03-01T00:00:00Z")
        vm.activeRuns = [active]
        // date-ok
        let detail = Self.makeDetail(runs: [
            Self.makeActiveRun(id: "run-old", createdAt: "2026-01-01T00:00:00Z")
        ])

        #expect(vm.currentRun(for: detail)?.id == "run-active")
    }

    @Test("currentRun falls back to the most recently created run when none is active")
    func currentRunFallsBackToMostRecent() async {
        let mock = MockNetworkClient()
        let vm = ProtocolsViewModel(networkClient: mock)
        vm.activeRuns = []
        // date-ok
        let detail = Self.makeDetail(runs: [
            Self.makeActiveRun(id: "run-older", createdAt: "2026-01-01T00:00:00Z"),
            Self.makeActiveRun(id: "run-newer", createdAt: "2026-02-01T00:00:00Z")
        ])

        #expect(vm.currentRun(for: detail)?.id == "run-newer")
    }

    @Test("currentRun returns nil when there are no runs at all")
    func currentRunNilWhenNoRuns() async {
        let mock = MockNetworkClient()
        let vm = ProtocolsViewModel(networkClient: mock)
        vm.activeRuns = []
        let detail = Self.makeDetail(runs: nil)

        #expect(vm.currentRun(for: detail) == nil)
    }

    // MARK: - Dose Action Error Surfacing

    @Test("logDose failure populates doseActionError with a status-specific message")
    func logDoseFailurePopulatesError() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in throw NetworkError.serverError(statusCode: 409, body: "conflict") }

        let vm = ProtocolsViewModel(networkClient: mock)
        let result = await vm.logDose(protocolId: "proto-1", runId: "run-1", lineId: "line-1", dayNumber: 0)

        #expect(result == false)
        #expect(vm.doseActionError?.contains("already been logged") == true)
    }

    @Test("logDose success clears any previous doseActionError")
    func logDoseSuccessClearsError() async {
        let mock = MockNetworkClient()
        mock.requestHandler = Self.doseActionHandler()

        let vm = ProtocolsViewModel(networkClient: mock)
        vm.doseActionError = "stale error from a previous attempt"
        await vm.logDose(protocolId: "proto-1", runId: "run-1", lineId: "line-1", dayNumber: 0)

        #expect(vm.doseActionError == nil)
    }

    // MARK: - Missed Doses

    @Test("loadMissedDoses success stores items")
    func loadMissedDosesSuccess() async {
        let mock = MockNetworkClient()
        let items = [
            MissedDoseItem(
                protocolId: "proto-1", protocolName: "Stack", runId: "run-1", protocolLineId: "line-1",
                substance: "Creatine", dose: 5.0, unit: "g", route: "oral", timeOfDay: nil,
                // date-ok
                dayNumber: 2, date: "2026-04-03", status: .missed
            )
        ]
        mock.requestHandler = { _, _, _ in items }

        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.loadMissedDoses()

        #expect(vm.missedDosesState == .loaded)
        #expect(vm.missedDoses.count == 1)
        #expect(mock.requestCalls[0].path == Endpoints.missedDoses)
    }

    @Test("loadMissedDoses failure transitions to error state")
    func loadMissedDosesFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in throw NetworkError.unauthorized }

        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.loadMissedDoses()

        if case .error = vm.missedDosesState {
            // expected
        } else {
            Issue.record("Expected error state")
        }
    }

    // MARK: - Delete / Undo Dose

    @Test("deleteDose success returns true")
    func deleteDoseSuccess() async {
        let mock = MockNetworkClient()
        mock.requestNoContentHandler = { _, _, _ in }

        let vm = ProtocolsViewModel(networkClient: mock)
        let result = await vm.deleteDose(runId: "run-1", doseId: "dose-1")

        #expect(result == true)
        #expect(mock.requestCalls[0].method == "DELETE")
        #expect(mock.requestCalls[0].path == Endpoints.deleteDose(runId: "run-1", doseId: "dose-1"))
    }

    @Test("deleteDose failure returns false")
    func deleteDoseFailure() async {
        let mock = MockNetworkClient()
        mock.requestNoContentHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 404, body: "not found")
        }

        let vm = ProtocolsViewModel(networkClient: mock)
        let result = await vm.deleteDose(runId: "run-1", doseId: "dose-1")

        #expect(result == false)
    }

    @Test("undoDose success refreshes adherence/doses/missed and the protocol")
    func undoDoseSuccess() async {
        let mock = MockNetworkClient()
        mock.requestNoContentHandler = { _, _, _ in }
        mock.requestHandler = Self.doseActionHandler()

        let vm = ProtocolsViewModel(networkClient: mock)
        let result = await vm.undoDose(protocolId: "proto-1", runId: "run-1", doseId: "dose-1")

        #expect(result == true)
        #expect(mock.requestCalls[0].method == "DELETE")
        // Delete + adherence + run-doses + missed-doses + quiet protocol re-fetch.
        #expect(mock.requestCalls.count == 5)
        #expect(mock.requestCalls.dropFirst().allSatisfy { $0.method == "GET" })
    }

    @Test("undoDose failure does not reload the protocol")
    func undoDoseFailure() async {
        let mock = MockNetworkClient()
        mock.requestNoContentHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 404, body: "not found")
        }

        let vm = ProtocolsViewModel(networkClient: mock)
        let result = await vm.undoDose(protocolId: "proto-1", runId: "run-1", doseId: "dose-1")

        #expect(result == false)
        #expect(mock.requestCalls.count == 1)
    }

    // MARK: - Log Dose sends tz_offset_minutes

    @Test("logDose always sends tz_offset_minutes and optional administeredAt/notes")
    func logDoseSendsTZOffsetAndBackfillFields() async {
        let mock = MockNetworkClient()
        var capturedBody: LogDoseRequest?
        let handler = Self.doseActionHandler()
        mock.requestHandler = { method, path, body in
            if method == "POST" && path.contains("doses/log") {
                capturedBody = body as? LogDoseRequest
            }
            return try handler(method, path, body)
        }

        let vm = ProtocolsViewModel(networkClient: mock)
        let backfillDate = Date(timeIntervalSince1970: 1_743_670_500) // 2025-04-03T09:15:00Z
        await vm.logDose(
            protocolId: "proto-1",
            runId: "run-1",
            lineId: "line-1",
            dayNumber: 3,
            administeredAt: backfillDate,
            notes: "logged a bit late"
        )

        let expectedOffset = TimeZone.current.secondsFromGMT() / 60
        #expect(capturedBody?.tzOffsetMinutes == expectedOffset)
        #expect(capturedBody?.notes == "logged a bit late")
        #expect(capturedBody?.administeredAt != nil)
    }

    @Test("skipDose sends an optional skip reason")
    func skipDoseSendsReason() async {
        let mock = MockNetworkClient()
        var capturedBody: SkipDoseRequest?
        mock.requestNoContentHandler = { _, _, body in
            capturedBody = body as? SkipDoseRequest
        }
        mock.requestHandler = Self.doseActionHandler()

        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.skipDose(protocolId: "proto-1", runId: "run-1", lineId: "line-1", dayNumber: 0, skipReason: "traveling")

        #expect(capturedBody?.skipReason == "traveling")
    }

    // MARK: - Reset Form

    @Test("resetCreateForm clears all fields")
    func resetForm() {
        let mock = MockNetworkClient()
        let vm = ProtocolsViewModel(networkClient: mock)

        vm.newName = "Test"
        vm.newDescription = "Desc"
        vm.newWeeks = 8
        vm.newLines[0].substance = "BPC-157"
        vm.addLine()
        vm.newLines[1].substance = "TB-500"

        vm.resetCreateForm()

        #expect(vm.newName == "")
        #expect(vm.newDescription == "")
        #expect(vm.newWeeks == 4)
        #expect(vm.newLines.count == 1)
        #expect(vm.newLines[0].substance == "")
    }
}
