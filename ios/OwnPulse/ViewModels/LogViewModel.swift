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
}
