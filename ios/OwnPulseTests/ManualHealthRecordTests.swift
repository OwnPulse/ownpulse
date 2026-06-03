// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

/// Tests for the five manual health-record entry forms' view-model logic:
/// validation and submit state transitions (success + failure paths).
@Suite("ManualHealthRecord", .serialized)
@MainActor
struct ManualHealthRecordTests {
    private func makeRecordResponse() -> HealthRecordResponse {
        HealthRecordResponse(
            id: "rec-1",
            userId: "user-1",
            source: "manual",
            recordType: "body_mass",
            value: 80,
            unit: "kg",
            startTime: Date(),
            endTime: Date()
        )
    }

    // MARK: - Weight

    @Test("weightIsValid requires a positive value")
    func weightValidation() {
        let vm = LogViewModel(networkClient: MockNetworkClient())
        #expect(vm.weightIsValid == false)
        vm.weightValue = "0"
        #expect(vm.weightIsValid == false)
        vm.weightValue = "-5"
        #expect(vm.weightIsValid == false)
        vm.weightValue = "abc"
        #expect(vm.weightIsValid == false)
        vm.weightValue = "82.5"
        #expect(vm.weightIsValid == true)
    }

    @Test("submitWeight success transitions to success and resets")
    func submitWeightSuccess() async {
        let mock = MockNetworkClient()
        let response = makeRecordResponse()
        mock.requestHandler = { _, _, _ in response }

        let vm = LogViewModel(networkClient: mock)
        vm.weightValue = "82.5"
        vm.weightUnit = "kg"

        await vm.submitWeight()

        #expect(vm.submitState == .success("Weight saved"))
        #expect(vm.weightValue == "")
        #expect(mock.requestCalls.count == 1)
        #expect(mock.requestCalls[0].method == "POST")
        #expect(mock.requestCalls[0].path == Endpoints.healthRecords)
    }

    @Test("submitWeight sends a manual-source body_mass record")
    func submitWeightBody() async {
        let mock = MockNetworkClient()
        var captured: CreateHealthRecord?
        mock.requestHandler = { _, _, body in
            captured = body as? CreateHealthRecord
            return self.makeRecordResponse()
        }

        let vm = LogViewModel(networkClient: mock)
        vm.weightValue = "75"
        vm.weightUnit = "kg"

        await vm.submitWeight()

        #expect(captured?.source == "manual")
        #expect(captured?.recordType == "body_mass")
        #expect(captured?.value == 75)
        #expect(captured?.unit == "kg")
    }

    @Test("submitWeight network failure transitions to error")
    func submitWeightFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "boom")
        }
        let vm = LogViewModel(networkClient: mock)
        vm.weightValue = "70"

        await vm.submitWeight()

        if case .error(let msg) = vm.submitState {
            #expect(msg.contains("Failed to save weight"))
        } else {
            Issue.record("Expected error state")
        }
        #expect(vm.weightValue == "70") // not reset on failure
    }

    @Test("submitWeight invalid input shows validation error without network call")
    func submitWeightValidation() async {
        let mock = MockNetworkClient()
        let vm = LogViewModel(networkClient: mock)

        await vm.submitWeight()

        #expect(vm.submitState == .error("Enter a valid weight"))
        #expect(mock.requestCalls.isEmpty)
    }

    // MARK: - Sleep

    @Test("sleepIsValid requires a positive total duration")
    func sleepValidation() {
        let vm = LogViewModel(networkClient: MockNetworkClient())
        #expect(vm.sleepIsValid == false) // both empty
        vm.sleepHours = "0"
        vm.sleepMinutes = "0"
        #expect(vm.sleepIsValid == false) // zero total
        vm.sleepHours = "7"
        vm.sleepMinutes = "30"
        #expect(vm.sleepIsValid == true)
        #expect(vm.sleepTotalMinutes == 450)
        vm.sleepHours = ""
        vm.sleepMinutes = "45"
        #expect(vm.sleepIsValid == true)
        #expect(vm.sleepTotalMinutes == 45)
    }

    @Test("sleepIsValid rejects minutes >= 60")
    func sleepMinutesBound() {
        let vm = LogViewModel(networkClient: MockNetworkClient())
        vm.sleepHours = "7"
        vm.sleepMinutes = "60"
        #expect(vm.sleepIsValid == false)
        vm.sleepMinutes = "75"
        #expect(vm.sleepIsValid == false)
    }

    @Test("submitSleep success stores total minutes and resets")
    func submitSleepSuccess() async {
        let mock = MockNetworkClient()
        var captured: CreateHealthRecord?
        mock.requestHandler = { _, _, body in
            captured = body as? CreateHealthRecord
            return self.makeRecordResponse()
        }
        let vm = LogViewModel(networkClient: mock)
        vm.sleepHours = "8"
        vm.sleepMinutes = "15"

        await vm.submitSleep()

        #expect(vm.submitState == .success("Sleep saved"))
        #expect(captured?.recordType == "sleep_analysis")
        #expect(captured?.value == 495)
        #expect(captured?.unit == "min")
        #expect(captured?.source == "manual")
        #expect(vm.sleepHours == "")
        #expect(vm.sleepMinutes == "")
    }

    @Test("submitSleep network failure transitions to error")
    func submitSleepFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in throw NetworkError.unauthorized }
        let vm = LogViewModel(networkClient: mock)
        vm.sleepHours = "7"

        await vm.submitSleep()

        if case .error(let msg) = vm.submitState {
            #expect(msg.contains("Failed to save sleep"))
        } else {
            Issue.record("Expected error state")
        }
    }

    @Test("submitSleep invalid input shows validation error")
    func submitSleepValidation() async {
        let mock = MockNetworkClient()
        let vm = LogViewModel(networkClient: mock)

        await vm.submitSleep()

        #expect(vm.submitState == .error("Enter a valid sleep duration"))
        #expect(mock.requestCalls.isEmpty)
    }

    // MARK: - Exercise

    @Test("exerciseIsValid requires positive minutes")
    func exerciseValidation() {
        let vm = LogViewModel(networkClient: MockNetworkClient())
        #expect(vm.exerciseIsValid == false)
        vm.exerciseMinutes = "0"
        #expect(vm.exerciseIsValid == false)
        vm.exerciseMinutes = "45"
        #expect(vm.exerciseIsValid == true)
    }

    @Test("submitExercise success stores exercise_time minutes and resets")
    func submitExerciseSuccess() async {
        let mock = MockNetworkClient()
        var captured: CreateHealthRecord?
        mock.requestHandler = { _, _, body in
            captured = body as? CreateHealthRecord
            return self.makeRecordResponse()
        }
        let vm = LogViewModel(networkClient: mock)
        vm.exerciseMinutes = "30"

        await vm.submitExercise()

        #expect(vm.submitState == .success("Exercise saved"))
        #expect(captured?.recordType == "exercise_time")
        #expect(captured?.value == 30)
        #expect(captured?.unit == "min")
        #expect(vm.exerciseMinutes == "")
    }

    @Test("submitExercise network failure transitions to error")
    func submitExerciseFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 422, body: "bad")
        }
        let vm = LogViewModel(networkClient: mock)
        vm.exerciseMinutes = "30"

        await vm.submitExercise()

        if case .error(let msg) = vm.submitState {
            #expect(msg.contains("Failed to save exercise"))
        } else {
            Issue.record("Expected error state")
        }
    }

    @Test("submitExercise invalid input shows validation error")
    func submitExerciseValidation() async {
        let mock = MockNetworkClient()
        let vm = LogViewModel(networkClient: mock)

        await vm.submitExercise()

        #expect(vm.submitState == .error("Enter a valid exercise duration"))
        #expect(mock.requestCalls.isEmpty)
    }

    // MARK: - Glucose

    @Test("glucoseIsValid requires positive value")
    func glucoseValidation() {
        let vm = LogViewModel(networkClient: MockNetworkClient())
        #expect(vm.glucoseIsValid == false)
        vm.glucoseValue = "0"
        #expect(vm.glucoseIsValid == false)
        vm.glucoseValue = "95"
        #expect(vm.glucoseIsValid == true)
    }

    @Test("submitGlucose success stores blood_glucose record and resets")
    func submitGlucoseSuccess() async {
        let mock = MockNetworkClient()
        var captured: CreateHealthRecord?
        mock.requestHandler = { _, _, body in
            captured = body as? CreateHealthRecord
            return self.makeRecordResponse()
        }
        let vm = LogViewModel(networkClient: mock)
        vm.glucoseValue = "95"
        vm.glucoseUnit = "mg/dL"

        await vm.submitGlucose()

        #expect(vm.submitState == .success("Glucose saved"))
        #expect(captured?.recordType == "blood_glucose")
        #expect(captured?.value == 95)
        #expect(captured?.unit == "mg/dL")
        #expect(vm.glucoseValue == "")
    }

    @Test("submitGlucose network failure transitions to error")
    func submitGlucoseFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in throw NetworkError.unauthorized }
        let vm = LogViewModel(networkClient: mock)
        vm.glucoseValue = "100"

        await vm.submitGlucose()

        if case .error(let msg) = vm.submitState {
            #expect(msg.contains("Failed to save glucose"))
        } else {
            Issue.record("Expected error state")
        }
    }

    @Test("submitGlucose invalid input shows validation error")
    func submitGlucoseValidation() async {
        let mock = MockNetworkClient()
        let vm = LogViewModel(networkClient: mock)

        await vm.submitGlucose()

        #expect(vm.submitState == .error("Enter a valid glucose reading"))
        #expect(mock.requestCalls.isEmpty)
    }

    // MARK: - Blood Pressure

    @Test("bloodPressureIsValid requires both readings with systolic > diastolic")
    func bloodPressureValidation() {
        let vm = LogViewModel(networkClient: MockNetworkClient())
        #expect(vm.bloodPressureIsValid == false)
        vm.systolicValue = "120"
        #expect(vm.bloodPressureIsValid == false) // diastolic missing
        vm.diastolicValue = "80"
        #expect(vm.bloodPressureIsValid == true)
        // systolic must exceed diastolic
        vm.systolicValue = "70"
        #expect(vm.bloodPressureIsValid == false)
        vm.systolicValue = "80"
        vm.diastolicValue = "80"
        #expect(vm.bloodPressureIsValid == false) // equal is invalid
    }

    @Test("submitBloodPressure success posts two records and resets")
    func submitBloodPressureSuccess() async {
        let mock = MockNetworkClient()
        var captured: [CreateHealthRecord] = []
        mock.requestHandler = { _, _, body in
            if let r = body as? CreateHealthRecord { captured.append(r) }
            return self.makeRecordResponse()
        }
        let vm = LogViewModel(networkClient: mock)
        vm.systolicValue = "120"
        vm.diastolicValue = "80"

        await vm.submitBloodPressure()

        #expect(vm.submitState == .success("Blood pressure saved"))
        #expect(mock.requestCalls.count == 2)
        #expect(captured.count == 2)
        #expect(captured[0].recordType == "blood_pressure_systolic")
        #expect(captured[0].value == 120)
        #expect(captured[1].recordType == "blood_pressure_diastolic")
        #expect(captured[1].value == 80)
        #expect(captured.allSatisfy { $0.unit == "mmHg" })
        #expect(captured.allSatisfy { $0.source == "manual" })
        #expect(vm.systolicValue == "")
        #expect(vm.diastolicValue == "")
    }

    @Test("submitBloodPressure network failure transitions to error")
    func submitBloodPressureFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "x")
        }
        let vm = LogViewModel(networkClient: mock)
        vm.systolicValue = "120"
        vm.diastolicValue = "80"

        await vm.submitBloodPressure()

        if case .error(let msg) = vm.submitState {
            #expect(msg.contains("Failed to save blood pressure"))
        } else {
            Issue.record("Expected error state")
        }
    }

    @Test("submitBloodPressure invalid input shows validation error")
    func submitBloodPressureValidation() async {
        let mock = MockNetworkClient()
        let vm = LogViewModel(networkClient: mock)
        vm.systolicValue = "80"
        vm.diastolicValue = "120" // inverted

        await vm.submitBloodPressure()

        #expect(vm.submitState == .error("Systolic must be greater than diastolic"))
        #expect(mock.requestCalls.isEmpty)
    }

    // MARK: - Tabs

    @Test("LogTab includes the five manual-entry tabs")
    func tabsPresent() {
        let raws = LogTab.allCases.map(\.rawValue)
        #expect(raws.contains("Weight"))
        #expect(raws.contains("Sleep"))
        #expect(raws.contains("Exercise"))
        #expect(raws.contains("Glucose"))
        #expect(raws.contains("BP"))
    }
}
