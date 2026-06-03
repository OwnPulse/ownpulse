// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Observation
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "log")

enum LogTab: String, CaseIterable, Sendable {
    case checkin = "Check-in"
    case intervention = "Intervention"
    case observation = "Observation"
    case weight = "Weight"
    case sleep = "Sleep"
    case exercise = "Exercise"
    case glucose = "Glucose"
    case bloodPressure = "BP"
}

enum ObservationType: String, CaseIterable, Sendable {
    case eventInstant = "event_instant"
    case eventDuration = "event_duration"
    case scale = "scale"
    case symptom = "symptom"
    case note = "note"
    case contextTag = "context_tag"
    case environmental = "environmental"

    var displayName: String {
        switch self {
        case .eventInstant: return "Event (Instant)"
        case .eventDuration: return "Event (Duration)"
        case .scale: return "Scale"
        case .symptom: return "Symptom"
        case .note: return "Note"
        case .contextTag: return "Context Tag"
        case .environmental: return "Environmental"
        }
    }
}

@Observable
@MainActor
final class LogViewModel {
    // MARK: - Common State

    enum SubmitState: Sendable, Equatable {
        case idle
        case submitting
        case success(String)
        case error(String)
    }

    var selectedTab: LogTab = .checkin
    var submitState: SubmitState = .idle

    // MARK: - Check-in State

    var checkinDate = Date()
    var energy = 5
    var mood = 5
    var focus = 5
    var recovery = 5
    var libido = 5
    var checkinNotes = ""

    // MARK: - Intervention State

    var substance = ""
    var dose: String = ""
    var doseUnit = "mg"
    var route = "oral"
    var interventionDate = Date()
    var fasted = false
    var interventionNotes = ""
    var savedMedicines: [SavedMedicine] = []

    static let doseUnits = ["mg", "mcg", "mL", "IU", "g", "drops", "puffs"]
    static let routes = ["oral", "sublingual", "subq", "IM", "IV", "topical", "inhaled", "nasal", "rectal", "transdermal"]

    // MARK: - Observation State

    var observationType: ObservationType = .eventInstant
    var observationName = ""
    var observationDate = Date()
    var observationEndDate = Date()
    var scaleValue = 5
    var scaleMax = 10
    var symptomSeverity = 5
    var noteText = ""
    var environmentalValue: String = ""
    var environmentalUnit = "celsius"
    var observationNotes = ""

    // MARK: - Manual Health Record State

    /// Source value for every manually-entered health record. The backend uses
    /// this to keep manual entries out of the HealthKit write-back cycle guard
    /// (only `source == "healthkit"` is excluded; "manual" is written back).
    static let manualSource = "manual"

    // Units are fixed to the canonical units declared in `HealthKitTypeMap`
    // for each record type. The write-back queue carries the stored unit
    // verbatim, so offering non-canonical units (e.g. "lb", "mmol/L") here
    // could write a wrong-unit value into Apple Health. If unit conversion is
    // added later, convert to canonical before POST — do not relax this.
    static let weightUnit = "kg"
    static let glucoseUnit = "mg/dL"

    // Weight
    var weightValue: String = ""
    var weightDate = Date()

    // Sleep — captured as hours + minutes, submitted as total minutes
    var sleepHours: String = ""
    var sleepMinutes: String = ""
    var sleepDate = Date()

    // Exercise — duration in minutes
    var exerciseMinutes: String = ""
    var exerciseDate = Date()

    // Glucose
    var glucoseValue: String = ""
    var glucoseDate = Date()

    // Blood pressure — submitted as two records (systolic + diastolic).
    // `bloodPressureSystolicSaved` records that the systolic POST already
    // succeeded in a prior submit that then failed on the diastolic POST, so
    // a retry resends only the diastolic and never duplicates the systolic.
    // Editing either reading after a partial save invalidates the
    // already-stored systolic record, so the flag is cleared and the next
    // submit posts a fresh systolic.
    var systolicValue: String = "" {
        didSet { if systolicValue != oldValue { bloodPressureSystolicSaved = false } }
    }
    var diastolicValue: String = "" {
        didSet { if diastolicValue != oldValue { bloodPressureSystolicSaved = false } }
    }
    var bloodPressureDate = Date()
    private(set) var bloodPressureSystolicSaved = false

    private let networkClient: NetworkClientProtocol

    init(networkClient: NetworkClientProtocol) {
        self.networkClient = networkClient
    }

    // MARK: - Validation

    var checkinIsValid: Bool {
        (1...10).contains(energy) &&
            (1...10).contains(mood) &&
            (1...10).contains(focus) &&
            (1...10).contains(recovery) &&
            (1...10).contains(libido)
    }

    var interventionIsValid: Bool {
        !substance.trimmingCharacters(in: .whitespaces).isEmpty &&
            Double(dose) != nil &&
            Double(dose)! > 0
    }

    var observationIsValid: Bool {
        !observationName.trimmingCharacters(in: .whitespaces).isEmpty
    }

    /// Parses a decimal string, returning a strictly-positive value or nil.
    private static func positiveValue(_ text: String) -> Double? {
        guard let v = Double(text.trimmingCharacters(in: .whitespaces)), v > 0 else { return nil }
        return v
    }

    var weightIsValid: Bool {
        Self.positiveValue(weightValue) != nil
    }

    /// Sleep is valid when at least one of hours/minutes is provided and the
    /// resulting total duration is positive. Minutes must be in 0..<60.
    var sleepTotalMinutes: Double? {
        let hoursText = sleepHours.trimmingCharacters(in: .whitespaces)
        let minutesText = sleepMinutes.trimmingCharacters(in: .whitespaces)
        let hours = hoursText.isEmpty ? 0 : Double(hoursText)
        let minutes = minutesText.isEmpty ? 0 : Double(minutesText)
        guard let h = hours, let m = minutes, h >= 0, m >= 0, m < 60 else { return nil }
        let total = h * 60 + m
        return total > 0 ? total : nil
    }

    var sleepIsValid: Bool {
        sleepTotalMinutes != nil
    }

    /// True when the minutes field holds a number outside 0..<60, so the form
    /// can show inline guidance (use the hours field for 60+).
    var sleepMinutesOutOfRange: Bool {
        let text = sleepMinutes.trimmingCharacters(in: .whitespaces)
        guard !text.isEmpty, let m = Double(text) else { return false }
        return m < 0 || m >= 60
    }

    var exerciseIsValid: Bool {
        Self.positiveValue(exerciseMinutes) != nil
    }

    var glucoseIsValid: Bool {
        Self.positiveValue(glucoseValue) != nil
    }

    /// Blood pressure requires both readings, positive, with systolic > diastolic.
    var bloodPressureIsValid: Bool {
        guard let sys = Self.positiveValue(systolicValue),
              let dia = Self.positiveValue(diastolicValue) else { return false }
        return sys > dia
    }

    // MARK: - Submit

    func submitCheckin() async {
        guard checkinIsValid else {
            submitState = .error("All scores must be between 1 and 10")
            return
        }

        submitState = .submitting

        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withFullDate]
        let body = UpsertCheckin(
            date: formatter.string(from: checkinDate),
            energy: energy,
            mood: mood,
            focus: focus,
            recovery: recovery,
            libido: libido,
            notes: checkinNotes.isEmpty ? nil : checkinNotes
        )

        do {
            let _: CheckinResponse = try await networkClient.request(
                method: "POST",
                path: Endpoints.checkins,
                body: body
            )
            submitState = .success("Check-in saved")
            resetCheckin()
        } catch {
            logger.error("Failed to submit checkin: \(error.localizedDescription, privacy: .public)")
            submitState = .error("Failed to save check-in: \(error.localizedDescription)")
        }
    }

    func submitIntervention() async {
        guard interventionIsValid else {
            submitState = .error("Substance name and a valid dose are required")
            return
        }

        submitState = .submitting

        let formatter = ISO8601DateFormatter()
        let body = CreateIntervention(
            substance: substance.trimmingCharacters(in: .whitespaces),
            dose: Double(dose) ?? 0,
            unit: doseUnit,
            route: route,
            administeredAt: formatter.string(from: interventionDate),
            fasted: fasted,
            notes: interventionNotes.isEmpty ? nil : interventionNotes
        )

        do {
            let _: InterventionResponse = try await networkClient.request(
                method: "POST",
                path: Endpoints.interventions,
                body: body
            )
            submitState = .success("Intervention logged")
            resetIntervention()
        } catch {
            logger.error("Failed to submit intervention: \(error.localizedDescription, privacy: .public)")
            submitState = .error("Failed to log intervention: \(error.localizedDescription)")
        }
    }

    func submitObservation() async {
        guard observationIsValid else {
            submitState = .error("Observation name is required")
            return
        }

        submitState = .submitting

        let formatter = ISO8601DateFormatter()
        var value: [String: AnyCodableValue] = [:]

        switch observationType {
        case .scale:
            value["numeric"] = .int(scaleValue)
            value["max"] = .int(scaleMax)
        case .symptom:
            value["severity"] = .int(symptomSeverity)
        case .note:
            value["text"] = .string(noteText)
        case .environmental:
            if let numVal = Double(environmentalValue) {
                value["numeric"] = .double(numVal)
                value["unit"] = .string(environmentalUnit)
            }
        case .eventInstant, .eventDuration, .contextTag:
            if !observationNotes.isEmpty {
                value["notes"] = .string(observationNotes)
            }
        }

        let endTime: String?
        if observationType == .eventDuration {
            endTime = formatter.string(from: observationEndDate)
        } else {
            endTime = nil
        }

        let body = CreateObservation(
            type: observationType.rawValue,
            name: observationName.trimmingCharacters(in: .whitespaces),
            startTime: formatter.string(from: observationDate),
            endTime: endTime,
            value: value
        )

        do {
            let _: ObservationResponse = try await networkClient.request(
                method: "POST",
                path: Endpoints.observations,
                body: body
            )
            submitState = .success("Observation logged")
            resetObservation()
        } catch {
            logger.error("Failed to submit observation: \(error.localizedDescription, privacy: .public)")
            submitState = .error("Failed to log observation: \(error.localizedDescription)")
        }
    }

    // MARK: - Submit Manual Health Records

    /// Posts a single manual `health_record`. Returns true on success.
    /// Date range is a single instant (start == end), matching how the
    /// HealthKit sync path encodes point-in-time samples.
    private func postHealthRecord(recordType: String, value: Double, unit: String, at date: Date) async throws {
        let body = CreateHealthRecord(
            source: Self.manualSource,
            recordType: recordType,
            value: value,
            unit: unit,
            startTime: date,
            endTime: date,
            metadata: nil,
            sourceId: nil
        )
        let _: HealthRecordResponse = try await networkClient.request(
            method: "POST",
            path: Endpoints.healthRecords,
            body: body
        )
    }

    func submitWeight() async {
        guard let value = Self.positiveValue(weightValue) else {
            submitState = .error("Enter a valid weight")
            return
        }
        submitState = .submitting
        do {
            try await postHealthRecord(recordType: "body_mass", value: value, unit: Self.weightUnit, at: weightDate)
            submitState = .success("Weight saved")
            resetWeight()
        } catch {
            logger.error("Failed to submit weight: \(error.localizedDescription, privacy: .public)")
            submitState = .error("Failed to save weight: \(error.localizedDescription)")
        }
    }

    func submitSleep() async {
        guard let totalMinutes = sleepTotalMinutes else {
            submitState = .error("Enter a valid sleep duration")
            return
        }
        submitState = .submitting
        do {
            try await postHealthRecord(recordType: "sleep_analysis", value: totalMinutes, unit: "min", at: sleepDate)
            submitState = .success("Sleep saved")
            resetSleep()
        } catch {
            logger.error("Failed to submit sleep: \(error.localizedDescription, privacy: .public)")
            submitState = .error("Failed to save sleep: \(error.localizedDescription)")
        }
    }

    func submitExercise() async {
        guard let minutes = Self.positiveValue(exerciseMinutes) else {
            submitState = .error("Enter a valid exercise duration")
            return
        }
        submitState = .submitting
        do {
            try await postHealthRecord(recordType: "exercise_time", value: minutes, unit: "min", at: exerciseDate)
            submitState = .success("Exercise saved")
            resetExercise()
        } catch {
            logger.error("Failed to submit exercise: \(error.localizedDescription, privacy: .public)")
            submitState = .error("Failed to save exercise: \(error.localizedDescription)")
        }
    }

    func submitGlucose() async {
        guard let value = Self.positiveValue(glucoseValue) else {
            submitState = .error("Enter a valid glucose reading")
            return
        }
        submitState = .submitting
        do {
            try await postHealthRecord(recordType: "blood_glucose", value: value, unit: Self.glucoseUnit, at: glucoseDate)
            submitState = .success("Glucose saved")
            resetGlucose()
        } catch {
            logger.error("Failed to submit glucose: \(error.localizedDescription, privacy: .public)")
            submitState = .error("Failed to save glucose: \(error.localizedDescription)")
        }
    }

    func submitBloodPressure() async {
        guard bloodPressureIsValid,
              let sys = Self.positiveValue(systolicValue),
              let dia = Self.positiveValue(diastolicValue) else {
            submitState = .error("Systolic must be greater than diastolic")
            return
        }
        submitState = .submitting

        // The systolic and diastolic readings are two separate POSTs. If a
        // prior submit saved systolic but then failed on diastolic, the flag
        // is set so this retry resends only the diastolic — never a second
        // systolic record. The flag is cleared once both halves are stored
        // (in resetBloodPressure) or when the user changes the readings.
        do {
            if !bloodPressureSystolicSaved {
                try await postHealthRecord(recordType: "blood_pressure_systolic", value: sys, unit: "mmHg", at: bloodPressureDate)
                bloodPressureSystolicSaved = true
            }
            try await postHealthRecord(recordType: "blood_pressure_diastolic", value: dia, unit: "mmHg", at: bloodPressureDate)
            submitState = .success("Blood pressure saved")
            resetBloodPressure()
        } catch {
            logger.error("Failed to submit blood pressure: \(error.localizedDescription, privacy: .public)")
            if bloodPressureSystolicSaved {
                // Systolic is persisted server-side; only the diastolic POST
                // failed. Tell the user so a retry completes the pair without
                // duplicating the systolic.
                submitState = .error("Systolic saved — failed to save diastolic. Tap to retry.")
            } else {
                submitState = .error("Failed to save blood pressure: \(error.localizedDescription)")
            }
        }
    }

    // MARK: - Saved Medicines

    func loadSavedMedicines() async {
        do {
            let medicines: [SavedMedicine] = try await networkClient.request(
                method: "GET",
                path: Endpoints.savedMedicines,
                body: nil as String?
            )
            savedMedicines = medicines
        } catch {
            logger.error("Failed to load saved medicines: \(error.localizedDescription, privacy: .public)")
        }
    }

    func saveMedicine() async {
        guard !substance.trimmingCharacters(in: .whitespaces).isEmpty else { return }

        let body = CreateSavedMedicine(
            substance: substance.trimmingCharacters(in: .whitespaces),
            dose: Double(dose),
            unit: doseUnit,
            route: route
        )

        do {
            let _: SavedMedicine = try await networkClient.request(
                method: "POST",
                path: Endpoints.savedMedicines,
                body: body
            )
            await loadSavedMedicines()
        } catch {
            logger.error("Failed to save medicine: \(error.localizedDescription, privacy: .public)")
        }
    }

    func deleteSavedMedicine(_ id: String) async {
        do {
            try await networkClient.requestNoContent(
                method: "DELETE",
                path: "\(Endpoints.savedMedicines)/\(id)",
                body: nil as String?
            )
            savedMedicines.removeAll { $0.id == id }
        } catch {
            logger.error("Failed to delete saved medicine: \(error.localizedDescription, privacy: .public)")
        }
    }

    func applySavedMedicine(_ medicine: SavedMedicine) {
        substance = medicine.substance
        if let d = medicine.dose { dose = String(d) }
        if let u = medicine.unit { doseUnit = u }
        if let r = medicine.route { route = r }
    }

    // MARK: - Reset

    private func resetCheckin() {
        energy = 5
        mood = 5
        focus = 5
        recovery = 5
        libido = 5
        checkinNotes = ""
        checkinDate = Date()
    }

    private func resetIntervention() {
        substance = ""
        dose = ""
        interventionNotes = ""
        interventionDate = Date()
        fasted = false
    }

    private func resetObservation() {
        observationName = ""
        noteText = ""
        observationNotes = ""
        environmentalValue = ""
        observationDate = Date()
        observationEndDate = Date()
    }

    private func resetWeight() {
        weightValue = ""
        weightDate = Date()
    }

    private func resetSleep() {
        sleepHours = ""
        sleepMinutes = ""
        sleepDate = Date()
    }

    private func resetExercise() {
        exerciseMinutes = ""
        exerciseDate = Date()
    }

    private func resetGlucose() {
        glucoseValue = ""
        glucoseDate = Date()
    }

    private func resetBloodPressure() {
        systolicValue = ""
        diastolicValue = ""
        bloodPressureDate = Date()
        bloodPressureSystolicSaved = false
    }
}
