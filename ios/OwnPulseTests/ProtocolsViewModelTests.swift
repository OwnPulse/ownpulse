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
            startDate: "2026-03-01",
            durationDays: durationDays,
            isTemplate: false,
            progressPct: progressPct,
            nextDose: nil,
            createdAt: "2026-03-01T00:00:00Z"
        )
    }

    private static func makeActiveRun(
        id: String = "run-1",
        protocolId: String = "proto-1",
        protocolName: String = "Test Protocol",
        progressPct: Double = 18.0,
        dosesToday: Int = 2,
        dosesCompletedToday: Int = 0
    ) -> ActiveRunResponse {
        ActiveRunResponse(
            id: id,
            protocolId: protocolId,
            protocolName: protocolName,
            startDate: "2026-03-28",
            durationDays: 28,
            status: "active",
            progressPct: progressPct,
            dosesToday: dosesToday,
            dosesCompletedToday: dosesCompletedToday,
            createdAt: "2026-03-28T10:00:00Z"
        )
    }

    private static func makeDetail(
        id: String = "proto-1",
        name: String = "Test Protocol",
        status: ProtocolStatus = .active,
        durationDays: Int = 28,
        lines: [ProtocolLine] = []
    ) -> ProtocolDetail {
        ProtocolDetail(
            id: id,
            userId: "user-1",
            name: name,
            description: "Test description",
            status: status,
            startDate: "2026-03-01",
            durationDays: durationDays,
            shareToken: nil,
            createdAt: "2026-03-01T00:00:00Z",
            updatedAt: "2026-03-01T00:00:00Z",
            lines: lines
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
            loggedAt: "2026-03-01T08:00:00Z",
            createdAt: "2026-03-01T08:00:00Z"
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

    // MARK: - Log Dose

    @Test("logDose with runId uses run endpoint")
    func logDoseWithRun() async {
        let mock = MockNetworkClient()
        let dose = Self.makeDose()
        let detail = Self.makeDetail()
        mock.requestHandler = { method, path, _ in
            if method == "POST" && path.contains("doses/log") {
                return dose
            }
            return detail
        }

        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.logDose(protocolId: "proto-1", runId: "run-1", lineId: "line-1", dayNumber: 0)

        #expect(mock.requestCalls.count == 2)
        #expect(mock.requestCalls[0].method == "POST")
        #expect(mock.requestCalls[0].path == Endpoints.runLogDose("run-1"))
        #expect(mock.requestCalls[1].method == "GET")
    }

    @Test("logDose without runId uses legacy endpoint")
    func logDoseLegacy() async {
        let mock = MockNetworkClient()
        let dose = Self.makeDose()
        let detail = Self.makeDetail()
        mock.requestHandler = { method, path, _ in
            if method == "POST" && path.contains("doses/log") {
                return dose
            }
            return detail
        }

        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.logDose(protocolId: "proto-1", runId: nil, lineId: "line-1", dayNumber: 0)

        #expect(mock.requestCalls.count == 2)
        #expect(mock.requestCalls[0].method == "POST")
        #expect(mock.requestCalls[0].path == Endpoints.protocolLogDose("proto-1"))
    }

    // MARK: - Skip Dose

    @Test("skipDose with runId uses run endpoint")
    func skipDoseWithRun() async {
        let mock = MockNetworkClient()
        let detail = Self.makeDetail()
        mock.requestNoContentHandler = { _, _, _ in }
        mock.requestHandler = { _, _, _ in detail }

        let vm = ProtocolsViewModel(networkClient: mock)
        await vm.skipDose(protocolId: "proto-1", runId: "run-1", lineId: "line-1", dayNumber: 0)

        #expect(mock.requestCalls.count == 2)
        #expect(mock.requestCalls[0].method == "POST")
        #expect(mock.requestCalls[0].path == Endpoints.runSkipDose("run-1"))
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
