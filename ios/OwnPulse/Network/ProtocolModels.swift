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
    /// Nullable — backend's `ProtocolResponse.start_date` is `Option<NaiveDate>`
    /// (draft protocols have no start date). The UI must treat nil as
    /// "not started yet".
    let startDate: String?
    let durationDays: Int
    let shareToken: String?
    let createdAt: String
    let lines: [ProtocolLine]

    enum CodingKeys: String, CodingKey {
        case id
        case userId = "user_id"
        case name, description, status
        case startDate = "start_date"
        case durationDays = "duration_days"
        case shareToken = "share_token"
        case createdAt = "created_at"
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
    /// Non-optional on the backend (`ProtocolDoseRow.logged_at: DateTime<Utc>`)
    /// but tolerated as optional here so older seeded rows without a
    /// populated timestamp don't break the whole detail decode.
    let loggedAt: String?

    enum CodingKeys: String, CodingKey {
        case id
        case protocolLineId = "protocol_line_id"
        case dayNumber = "day_number"
        case status
        case interventionId = "intervention_id"
        case loggedAt = "logged_at"
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
    /// Whether the user opted in to local dose reminders for this run.
    let notify: Bool
    /// Legacy single reminder time, `"HH:mm"`. Superseded by `notifyTimes`
    /// when present, but some runs only ever set this field.
    let notifyTime: String?
    /// One or more reminder times, `"HH:mm"` each. When both this and
    /// `notifyTime` are nil/empty but `notify` is true, a default reminder
    /// time is used — see `DoseReminderCoordinator`.
    let notifyTimes: [String]?
    /// Server-side "repeat until logged" setting. NOT implemented on-device
    /// — see `NotificationManager.scheduleDoseReminders`. Surfaced here only
    /// so the UI can reflect what was configured; it has no on-device effect.
    let repeatReminders: Bool
    let repeatIntervalMinutes: Int?
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
        case notify
        case notifyTime = "notify_time"
        case notifyTimes = "notify_times"
        case repeatReminders = "repeat_reminders"
        case repeatIntervalMinutes = "repeat_interval_minutes"
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

// MARK: - Active Substances (quick-pick on the Log form)

/// One entry per line across the user's currently active protocol runs,
/// used to pre-fill the intervention log form without retyping
/// dose/unit/route. Modeled on the backend's `ActiveSubstanceItem`
/// (`backend/api/src/models/protocol.rs`) — note that shape has no
/// `protocol_id` field (unlike web's `ActiveSubstance` TS interface in
/// `web/src/api/protocols.ts`, which declares one that the backend does not
/// actually serialize); `id` below is synthesized client-side instead.
struct ActiveSubstance: Codable, Sendable, Identifiable {
    var id: String { "\(protocolName)-\(substance)-\(dose ?? 0)-\(unit ?? "")" }
    let substance: String
    let dose: Double?
    let unit: String?
    let route: String?
    let protocolName: String

    enum CodingKeys: String, CodingKey {
        case substance, dose, unit, route
        case protocolName = "protocol_name"
    }
}

// MARK: - Endpoint Extensions

extension Endpoints {
    static let protocols = "/api/v1/protocols"
    static let activeRuns = "/api/v1/protocols/runs/active"
    static let activeSubstances = "/api/v1/protocols/active-substances"

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
