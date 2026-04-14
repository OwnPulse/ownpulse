// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Observation
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "protocols")

@Observable
@MainActor
final class ProtocolsViewModel {
    // MARK: - State

    enum LoadState: Sendable, Equatable {
        case idle
        case loading
        case loaded
        case error(String)
    }

    enum CreateState: Sendable, Equatable {
        case idle
        case submitting
        case success(String)
        case error(String)
    }

    // MARK: - List State

    var listState: LoadState = .idle
    var protocols: [ProtocolListItem] = []
    var activeRuns: [ActiveRunResponse] = []
    var filter: ProtocolFilter = .active

    enum ProtocolFilter: String, CaseIterable, Sendable {
        case active = "Active"
        case completed = "Completed"
        case all = "All"
    }

    var filteredProtocols: [ProtocolListItem] {
        switch filter {
        case .active:
            return protocols.filter { $0.status == .active || $0.status == .paused || $0.status == .draft }
        case .completed:
            return protocols.filter { $0.status == .completed || $0.status == .archived }
        case .all:
            return protocols
        }
    }

    // MARK: - Detail State

    var detailState: LoadState = .idle
    var selectedProtocol: ProtocolDetail?

    // MARK: - Create State

    var createState: CreateState = .idle
    var newName = ""
    var newDescription = ""
    var newStartDate = Date()
    var newWeeks = 4
    var newLines: [LineFormState] = [LineFormState()]

    var newDurationDays: Int { newWeeks * 7 }

    var createIsValid: Bool {
        !newName.trimmingCharacters(in: .whitespaces).isEmpty
            && newLines.allSatisfy { !$0.substance.trimmingCharacters(in: .whitespaces).isEmpty }
            && !newLines.isEmpty
    }

    // MARK: - Dependencies

    private let networkClient: NetworkClientProtocol

    init(networkClient: NetworkClientProtocol) {
        self.networkClient = networkClient
    }

    // MARK: - List

    func loadProtocols() async {
        listState = .loading

        do {
            async let fetchProtocols: [ProtocolListItem] = networkClient.request(
                method: "GET",
                path: Endpoints.protocols,
                body: nil as String?
            )
            async let fetchRuns: [ActiveRunResponse] = networkClient.request(
                method: "GET",
                path: Endpoints.activeRuns,
                body: nil as String?
            )
            let (items, runs) = try await (fetchProtocols, fetchRuns)
            protocols = items
            activeRuns = runs
            listState = .loaded
        } catch {
            logger.error("Failed to load protocols: \(error.localizedDescription, privacy: .public)")
            listState = .error("Failed to load protocols")
        }
    }

    func startRun(protocolId: String) async -> Bool {
        let body = StartRunRequest(startDate: formatDate(Date()), notify: false)
        do {
            let _: ActiveRunResponse = try await networkClient.request(
                method: "POST",
                path: Endpoints.protocolRuns(protocolId),
                body: body
            )
            return true
        } catch {
            logger.error("Failed to start run: \(error.localizedDescription, privacy: .public)")
            return false
        }
    }

    // MARK: - Detail

    func loadProtocol(id: String) async {
        detailState = .loading

        do {
            let detail: ProtocolDetail = try await networkClient.request(
                method: "GET",
                path: Endpoints.protocolDetail(id),
                body: nil as String?
            )
            selectedProtocol = detail
            detailState = .loaded
        } catch {
            logger.error("Failed to load protocol: \(error.localizedDescription, privacy: .public)")
            detailState = .error("Failed to load protocol")
        }
    }

    // MARK: - Create

    func createProtocol() async {
        guard createIsValid else {
            createState = .error("Name and at least one substance are required")
            return
        }

        createState = .submitting

        let lines = newLines.enumerated().map { index, line -> CreateProtocolLineRequest in
            let pattern = buildSchedulePattern(
                from: line.patternType,
                durationDays: newDurationDays
            )
            return CreateProtocolLineRequest(
                substance: line.substance.trimmingCharacters(in: .whitespaces),
                dose: Double(line.dose),
                unit: line.unit.isEmpty ? nil : line.unit,
                route: line.route.isEmpty ? nil : line.route,
                timeOfDay: line.timeOfDay.isEmpty ? nil : line.timeOfDay,
                schedulePattern: pattern,
                sortOrder: index
            )
        }

        let body = CreateProtocolRequest(
            name: newName.trimmingCharacters(in: .whitespaces),
            description: newDescription.isEmpty ? nil : newDescription,
            startDate: formatDate(newStartDate),
            durationDays: newDurationDays,
            lines: lines
        )

        do {
            let _: ProtocolDetail = try await networkClient.request(
                method: "POST",
                path: Endpoints.protocols,
                body: body
            )
            createState = .success("Protocol created")
            resetCreateForm()
        } catch {
            logger.error("Failed to create protocol: \(error.localizedDescription, privacy: .public)")
            createState = .error("Failed to create protocol: \(error.localizedDescription)")
        }
    }

    // MARK: - Dose Actions

    func logDose(protocolId: String, runId: String?, lineId: String, dayNumber: Int) async {
        let body = LogDoseRequest(protocolLineId: lineId, dayNumber: dayNumber)
        do {
            if let runId {
                let _: ProtocolDose = try await networkClient.request(
                    method: "POST",
                    path: Endpoints.runLogDose(runId),
                    body: body
                )
            } else {
                let _: ProtocolDose = try await networkClient.request(
                    method: "POST",
                    path: Endpoints.protocolLogDose(protocolId),
                    body: body
                )
            }
            await loadProtocol(id: protocolId)
        } catch {
            logger.error("Failed to log dose: \(error.localizedDescription, privacy: .public)")
        }
    }

    func skipDose(protocolId: String, runId: String?, lineId: String, dayNumber: Int) async {
        let body = SkipDoseRequest(protocolLineId: lineId, dayNumber: dayNumber)
        do {
            if let runId {
                try await networkClient.requestNoContent(
                    method: "POST",
                    path: Endpoints.runSkipDose(runId),
                    body: body
                )
            } else {
                try await networkClient.requestNoContent(
                    method: "POST",
                    path: Endpoints.protocolSkipDose(protocolId),
                    body: body
                )
            }
            await loadProtocol(id: protocolId)
        } catch {
            logger.error("Failed to skip dose: \(error.localizedDescription, privacy: .public)")
        }
    }

    // MARK: - Edit

    func updateProtocol(id: String, name: String?, description: String?, status: String?) async -> Bool {
        let body = UpdateProtocolRequest(name: name, description: description, status: status)
        do {
            try await networkClient.requestNoContent(
                method: "PATCH",
                path: Endpoints.protocolDetail(id),
                body: body
            )
            return true
        } catch {
            logger.error("Failed to update protocol: \(error.localizedDescription, privacy: .public)")
            return false
        }
    }

    // MARK: - Delete

    func deleteProtocol(id: String) async -> Bool {
        do {
            try await networkClient.requestNoContent(
                method: "DELETE",
                path: Endpoints.protocolDetail(id),
                body: nil as String?
            )
            return true
        } catch {
            logger.error("Failed to delete protocol: \(error.localizedDescription, privacy: .public)")
            return false
        }
    }

    // MARK: - Line Management

    func addLine() {
        newLines.append(LineFormState())
    }

    func removeLine(at index: Int) {
        guard newLines.count > 1 else { return }
        newLines.remove(at: index)
    }

    // MARK: - Helpers

    private func formatDate(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.locale = Locale(identifier: "en_US_POSIX")
        return formatter.string(from: date)
    }

    private func buildSchedulePattern(from type: PatternType, durationDays: Int) -> [Bool] {
        switch type {
        case .daily:
            return Array(repeating: true, count: durationDays)
        case .everyOtherDay:
            return (0..<durationDays).map { $0 % 2 == 0 }
        case .weekdaysOnly:
            // Start from the start date; approximate with Mon-Fri pattern
            return (0..<durationDays).map { day in
                let weekday = (day % 7)
                // 0=start day; we just use a 5-on/2-off pattern
                return weekday < 5
            }
        case .threeTimesWeek:
            // Mon, Wed, Fri pattern
            return (0..<durationDays).map { day in
                let weekday = day % 7
                return weekday == 0 || weekday == 2 || weekday == 4
            }
        }
    }

    func resetCreateForm() {
        newName = ""
        newDescription = ""
        newStartDate = Date()
        newWeeks = 4
        newLines = [LineFormState()]
    }

    // MARK: - Helpers

    func activeRun(for protocolId: String) -> ActiveRunResponse? {
        activeRuns.first { $0.protocolId == protocolId }
    }
}

// MARK: - Line Form State

struct LineFormState: Sendable {
    var substance = ""
    var dose = ""
    var unit = "mg"
    var route = ""
    var timeOfDay = ""
    var patternType: PatternType = .daily
}

enum PatternType: String, CaseIterable, Sendable {
    case daily = "Daily"
    case everyOtherDay = "Every Other Day"
    case weekdaysOnly = "Weekdays Only"
    case threeTimesWeek = "3x/Week"
}
