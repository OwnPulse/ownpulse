// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

// MARK: - Protocol List Item

struct ProtocolListItem: Codable, Sendable, Identifiable {
    let id: String
    let name: String
    let status: ProtocolStatus
    let startDate: String?
    let durationDays: Int
    let isTemplate: Bool?
    let progressPct: Double
    let nextDose: String?
    let createdAt: String

    enum CodingKeys: String, CodingKey {
        case id, name, status
        case startDate = "start_date"
        case durationDays = "duration_days"
        case isTemplate = "is_template"
        case progressPct = "progress_pct"
        case nextDose = "next_dose"
        case createdAt = "created_at"
    }
}

// MARK: - Protocol Detail

struct ProtocolDetail: Codable, Sendable, Identifiable {
    let id: String
    let userId: String?
    let name: String
    let description: String?
    let status: ProtocolStatus
    let startDate: String
    let durationDays: Int
    let shareToken: String?
    let createdAt: String
    let updatedAt: String
    let lines: [ProtocolLine]

    enum CodingKeys: String, CodingKey {
        case id
        case userId = "user_id"
        case name, description, status
        case startDate = "start_date"
        case durationDays = "duration_days"
        case shareToken = "share_token"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
        case lines
    }
}

// MARK: - Protocol Line

struct ProtocolLine: Codable, Sendable, Identifiable {
    let id: String
    let protocolId: String
    let substance: String
    let dose: Double?
    let unit: String?
    let route: String?
    let timeOfDay: String?
    let schedulePattern: [Bool]
    let sortOrder: Int
    let doses: [ProtocolDose]

    enum CodingKeys: String, CodingKey {
        case id
        case protocolId = "protocol_id"
        case substance, dose, unit, route
        case timeOfDay = "time_of_day"
        case schedulePattern = "schedule_pattern"
        case sortOrder = "sort_order"
        case doses
    }
}

// MARK: - Protocol Dose

struct ProtocolDose: Codable, Sendable, Identifiable {
    let id: String
    let protocolLineId: String
    let dayNumber: Int
    let status: DoseStatus
    let interventionId: String?
    let loggedAt: String?
    let createdAt: String

    enum CodingKeys: String, CodingKey {
        case id
        case protocolLineId = "protocol_line_id"
        case dayNumber = "day_number"
        case status
        case interventionId = "intervention_id"
        case loggedAt = "logged_at"
        case createdAt = "created_at"
    }
}

// MARK: - Enums

enum ProtocolStatus: String, Codable, Sendable, CaseIterable {
    case active
    case paused
    case completed
    case draft
    case archived
}

enum DoseStatus: String, Codable, Sendable {
    case completed
    case skipped
    case pending
}

// MARK: - Create Protocol

struct CreateProtocolRequest: Codable, Sendable {
    let name: String
    let description: String?
    let startDate: String
    let durationDays: Int
    let lines: [CreateProtocolLineRequest]

    enum CodingKeys: String, CodingKey {
        case name, description
        case startDate = "start_date"
        case durationDays = "duration_days"
        case lines
    }
}

struct CreateProtocolLineRequest: Codable, Sendable {
    let substance: String
    let dose: Double?
    let unit: String?
    let route: String?
    let timeOfDay: String?
    let schedulePattern: [Bool]
    let sortOrder: Int

    enum CodingKeys: String, CodingKey {
        case substance, dose, unit, route
        case timeOfDay = "time_of_day"
        case schedulePattern = "schedule_pattern"
        case sortOrder = "sort_order"
    }
}

// MARK: - Log/Skip Dose

struct LogDoseRequest: Codable, Sendable {
    let protocolLineId: String
    let dayNumber: Int

    enum CodingKeys: String, CodingKey {
        case protocolLineId = "protocol_line_id"
        case dayNumber = "day_number"
    }
}

struct SkipDoseRequest: Codable, Sendable {
    let protocolLineId: String
    let dayNumber: Int

    enum CodingKeys: String, CodingKey {
        case protocolLineId = "protocol_line_id"
        case dayNumber = "day_number"
    }
}

// MARK: - Update Protocol

struct UpdateProtocolRequest: Codable, Sendable {
    let name: String?
    let description: String?
    let status: String?
}

// MARK: - Active Run

struct ActiveRunResponse: Codable, Sendable, Identifiable {
    let id: String
    let protocolId: String
    let protocolName: String?
    let startDate: String
    let durationDays: Int?
    let status: String
    let progressPct: Double
    let dosesToday: Int
    let dosesCompletedToday: Int
    let createdAt: String

    enum CodingKeys: String, CodingKey {
        case id
        case protocolId = "protocol_id"
        case protocolName = "protocol_name"
        case startDate = "start_date"
        case durationDays = "duration_days"
        case status
        case progressPct = "progress_pct"
        case dosesToday = "doses_today"
        case dosesCompletedToday = "doses_completed_today"
        case createdAt = "created_at"
    }
}

// MARK: - Start Run

struct StartRunRequest: Codable, Sendable {
    let startDate: String?
    let notify: Bool?

    enum CodingKeys: String, CodingKey {
        case startDate = "start_date"
        case notify
    }
}

// MARK: - Endpoint Extensions

extension Endpoints {
    static let protocols = "/api/v1/protocols"
    static let activeRuns = "/api/v1/protocols/runs/active"

    static func protocolDetail(_ id: String) -> String {
        "/api/v1/protocols/\(id)"
    }

    static func protocolRuns(_ protocolId: String) -> String {
        "/api/v1/protocols/\(protocolId)/runs"
    }

    static func runLogDose(_ runId: String) -> String {
        "/api/v1/protocols/runs/\(runId)/doses/log"
    }

    static func runSkipDose(_ runId: String) -> String {
        "/api/v1/protocols/runs/\(runId)/doses/skip"
    }

    static func protocolLogDose(_ protocolId: String) -> String {
        "/api/v1/protocols/\(protocolId)/doses/log"
    }

    static func protocolSkipDose(_ protocolId: String) -> String {
        "/api/v1/protocols/\(protocolId)/doses/skip"
    }
}
