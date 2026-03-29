// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("LogViewModel", .serialized)
@MainActor
struct LogViewModelTests {
    // MARK: - Check-in Validation

    @Test("checkinIsValid returns true for default values")
    func checkinDefaultsValid() {
        let mock = MockNetworkClient()
        let vm = LogViewModel(networkClient: mock)
        #expect(vm.checkinIsValid == true)
    }

    @Test("checkinIsValid returns false for out-of-range energy")
    func checkinInvalidEnergy() {
        let mock = MockNetworkClient()
        let vm = LogViewModel(networkClient: mock)
        vm.energy = 0
        #expect(vm.checkinIsValid == false)

        vm.energy = 11
        #expect(vm.checkinIsValid == false)
    }

    // MARK: - Intervention Validation

    @Test("interventionIsValid requires substance and positive dose")
    func interventionValidation() {
        let mock = MockNetworkClient()
        let vm = LogViewModel(networkClient: mock)

        #expect(vm.interventionIsValid == false) // empty substance

        vm.substance = "Caffeine"
        #expect(vm.interventionIsValid == false) // empty dose

        vm.dose = "0"
        #expect(vm.interventionIsValid == false) // zero dose

        vm.dose = "200"
        #expect(vm.interventionIsValid == true)
    }

    @Test("interventionIsValid rejects whitespace-only substance")
    func interventionWhitespaceSubstance() {
        let mock = MockNetworkClient()
        let vm = LogViewModel(networkClient: mock)
        vm.substance = "   "
        vm.dose = "100"
        #expect(vm.interventionIsValid == false)
    }

    // MARK: - Observation Validation

    @Test("observationIsValid requires non-empty name")
    func observationValidation() {
        let mock = MockNetworkClient()
        let vm = LogViewModel(networkClient: mock)

        #expect(vm.observationIsValid == false)

        vm.observationName = "Cold plunge"
        #expect(vm.observationIsValid == true)
    }

    // MARK: - Submit Check-in

    @Test("submitCheckin success transitions to success state and resets")
    func submitCheckinSuccess() async {
        let mock = MockNetworkClient()
        let response = CheckinResponse(
            id: "checkin-1", date: "2026-03-28", energy: 7, mood: 8,
            focus: 6, recovery: 7, libido: 5
        )
        mock.requestHandler = { _, _, _ in response }

        let vm = LogViewModel(networkClient: mock)
        vm.energy = 7
        vm.mood = 8

        await vm.submitCheckin()

        #expect(vm.submitState == .success("Check-in saved"))
        // Verify reset
        #expect(vm.energy == 5)
        #expect(vm.mood == 5)
        // Verify network call
        #expect(mock.requestCalls.count == 1)
        #expect(mock.requestCalls[0].method == "POST")
        #expect(mock.requestCalls[0].path == Endpoints.checkins)
    }

    @Test("submitCheckin failure transitions to error state")
    func submitCheckinFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "internal")
        }

        let vm = LogViewModel(networkClient: mock)

        await vm.submitCheckin()

        if case .error(let msg) = vm.submitState {
            #expect(msg.contains("Failed to save check-in"))
        } else {
            Issue.record("Expected error state")
        }
    }

    @Test("submitCheckin with invalid scores shows validation error")
    func submitCheckinValidationError() async {
        let mock = MockNetworkClient()
        let vm = LogViewModel(networkClient: mock)
        vm.energy = 0 // invalid

        await vm.submitCheckin()

        #expect(vm.submitState == .error("All scores must be between 1 and 10"))
        #expect(mock.requestCalls.isEmpty) // no network call made
    }

    @Test("submitCheckin sends correct request body")
    func submitCheckinRequestBody() async {
        let mock = MockNetworkClient()
        let response = CheckinResponse(
            id: "c-1", date: "2026-03-28", energy: 8, mood: 9,
            focus: 7, recovery: 6, libido: 5
        )

        var capturedBody: UpsertCheckin?
        mock.requestHandler = { _, _, body in
            if let checkin = body as? UpsertCheckin {
                capturedBody = checkin
            }
            return response
        }

        let vm = LogViewModel(networkClient: mock)
        vm.energy = 8
        vm.mood = 9
        vm.focus = 7
        vm.recovery = 6
        vm.libido = 5
        vm.checkinNotes = "Felt great"

        await vm.submitCheckin()

        #expect(capturedBody?.energy == 8)
        #expect(capturedBody?.mood == 9)
        #expect(capturedBody?.notes == "Felt great")
    }

    // MARK: - Submit Intervention

    @Test("submitIntervention success transitions to success state")
    func submitInterventionSuccess() async {
        let mock = MockNetworkClient()
        let response = InterventionResponse(id: "int-1", substance: "Caffeine")
        mock.requestHandler = { _, _, _ in response }

        let vm = LogViewModel(networkClient: mock)
        vm.substance = "Caffeine"
        vm.dose = "200"
        vm.doseUnit = "mg"
        vm.route = "oral"

        await vm.submitIntervention()

        #expect(vm.submitState == .success("Intervention logged"))
        #expect(vm.substance == "") // reset
        #expect(mock.requestCalls[0].path == Endpoints.interventions)
    }

    @Test("submitIntervention failure transitions to error state")
    func submitInterventionFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 422, body: "validation")
        }

        let vm = LogViewModel(networkClient: mock)
        vm.substance = "Melatonin"
        vm.dose = "3"

        await vm.submitIntervention()

        if case .error(let msg) = vm.submitState {
            #expect(msg.contains("Failed to log intervention"))
        } else {
            Issue.record("Expected error state")
        }
    }

    @Test("submitIntervention with invalid data shows validation error")
    func submitInterventionValidationError() async {
        let mock = MockNetworkClient()
        let vm = LogViewModel(networkClient: mock)
        // Empty substance and dose

        await vm.submitIntervention()

        #expect(vm.submitState == .error("Substance name and a valid dose are required"))
        #expect(mock.requestCalls.isEmpty)
    }

    // MARK: - Submit Observation

    @Test("submitObservation success transitions to success state")
    func submitObservationSuccess() async {
        let mock = MockNetworkClient()
        let response = ObservationResponse(id: "obs-1", type: "event_instant", name: "Sauna")
        mock.requestHandler = { _, _, _ in response }

        let vm = LogViewModel(networkClient: mock)
        vm.observationName = "Sauna"
        vm.observationType = .eventInstant

        await vm.submitObservation()

        #expect(vm.submitState == .success("Observation logged"))
        #expect(vm.observationName == "") // reset
        #expect(mock.requestCalls[0].path == Endpoints.observations)
    }

    @Test("submitObservation failure transitions to error state")
    func submitObservationFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.unauthorized
        }

        let vm = LogViewModel(networkClient: mock)
        vm.observationName = "Walk"

        await vm.submitObservation()

        if case .error(let msg) = vm.submitState {
            #expect(msg.contains("Failed to log observation"))
        } else {
            Issue.record("Expected error state")
        }
    }

    @Test("submitObservation with empty name shows validation error")
    func submitObservationValidationError() async {
        let mock = MockNetworkClient()
        let vm = LogViewModel(networkClient: mock)

        await vm.submitObservation()

        #expect(vm.submitState == .error("Observation name is required"))
        #expect(mock.requestCalls.isEmpty)
    }

    // MARK: - Tab Switching

    @Test("selectedTab defaults to checkin")
    func defaultTab() {
        let mock = MockNetworkClient()
        let vm = LogViewModel(networkClient: mock)
        #expect(vm.selectedTab == .checkin)
    }

    @Test("selectedTab can switch between all tabs")
    func switchTabs() {
        let mock = MockNetworkClient()
        let vm = LogViewModel(networkClient: mock)

        vm.selectedTab = .intervention
        #expect(vm.selectedTab == .intervention)

        vm.selectedTab = .observation
        #expect(vm.selectedTab == .observation)

        vm.selectedTab = .checkin
        #expect(vm.selectedTab == .checkin)
    }
}
