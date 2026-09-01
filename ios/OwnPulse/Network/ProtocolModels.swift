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
    /// Embedded runs for this protocol (`ProtocolResponse.runs` on the
    /// backend). Optional so older fixtures/decodes without the field don't
    /// break — used to fall back to the most-recently-created run when there
    /// is no *active* run (e.g. a paused run), matching the backend's own
    /// active-else-most-recent scoping in `get_by_id`/`get_shared`.
    ///
    /// `var`, not `let`, deliberately: a stored `let` with an inline default
    /// value is excluded entirely from Swift's synthesized memberwise
    /// init — callers couldn't pass `runs:` at all, only ever get `nil`. A
    /// `var` with a default is included as an *optional* init parameter,
    /// which is what every test constructing this type by hand needs. Never
    /// mutated after decode/init.
    var runs: [ActiveRunResponse]? = nil

    enum CodingKeys: String, CodingKey {
        case id
        case userId = "user_id"
        case name, description, status
        case startDate = "start_date"
        case durationDays = "duration_days"
        case shareToken = "share_token"
        case createdAt = "created_at"
        case lines, runs
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
    /// Present on the `doses/log` and `doses/skip` response bodies
    /// (`run_id`/`skip_reason` per api.md) but absent on the doses embedded
    /// in `GET /protocols/:id`'s `lines[].doses` — optional so this one type
    /// decodes both shapes.
    let runId: String?
    let skipReason: String?

    enum CodingKeys: String, CodingKey {
        case id
        case protocolLineId = "protocol_line_id"
        case dayNumber = "day_number"
        case status
        case interventionId = "intervention_id"
        case loggedAt = "logged_at"
        case runId = "run_id"
        case skipReason = "skip_reason"
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
    case missed
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
    /// Optional backfill timestamp. Must fall within one calendar day of
    /// `start_date + day_number` (evaluated in `tzOffsetMinutes`) or the
    /// server returns 400. Also used when quick-picking a substance that
    /// matches today's pending dose, so the log reflects the form's chosen
    /// date/time rather than the server's default.
    let administeredAt: String?
    let notes: String?
    /// Always sent — the caller's local UTC offset in minutes, so the
    /// server evaluates date-boundary comparisons in the user's own
    /// calendar day rather than UTC's. See docs/architecture/api.md.
    let tzOffsetMinutes: Int

    enum CodingKeys: String, CodingKey {
        case protocolLineId = "protocol_line_id"
        case dayNumber = "day_number"
        case administeredAt = "administered_at"
        case notes
        case tzOffsetMinutes = "tz_offset_minutes"
    }
}

struct SkipDoseRequest: Codable, Sendable {
    let protocolLineId: String
    let dayNumber: Int
    let skipReason: String?

    enum CodingKeys: String, CodingKey {
        case protocolLineId = "protocol_line_id"
        case dayNumber = "day_number"
        case skipReason = "skip_reason"
    }
}

// MARK: - Adherence

struct AdherenceLineResponse: Codable, Sendable, Identifiable {
    var id: String { protocolLineId }
    let protocolLineId: String
    let substance: String
    let scheduledSoFar: Int
    let completed: Int
    let skipped: Int
    let missed: Int
    let adherencePct: Double?

    enum CodingKeys: String, CodingKey {
        case protocolLineId = "protocol_line_id"
        case substance
        case scheduledSoFar = "scheduled_so_far"
        case completed, skipped, missed
        case adherencePct = "adherence_pct"
    }
}

struct AdherenceResponse: Codable, Sendable {
    let runId: String
    let scheduledSoFar: Int
    let completed: Int
    let skipped: Int
    let missed: Int
    let adherencePct: Double?
    let lines: [AdherenceLineResponse]

    enum CodingKeys: String, CodingKey {
        case runId = "run_id"
        case scheduledSoFar = "scheduled_so_far"
        case completed, skipped, missed
        case adherencePct = "adherence_pct"
        case lines
    }
}

// MARK: - Run Dose Day (GET /protocols/runs/:run_id/doses)

struct RunDoseDay: Codable, Sendable, Identifiable {
    var id: String { "\(protocolLineId)-\(dayNumber)" }
    let dayNumber: Int
    let date: String
    let protocolLineId: String
    let substance: String
    let dose: Double?
    let unit: String?
    let route: String?
    let timeOfDay: String?
    let status: DoseStatus
    let doseId: String?
    let interventionId: String?
    let skipReason: String?
    let loggedAt: String?

    enum CodingKeys: String, CodingKey {
        case dayNumber = "day_number"
        case date
        case protocolLineId = "protocol_line_id"
        case substance, dose, unit, route
        case timeOfDay = "time_of_day"
        case status
        case doseId = "dose_id"
        case interventionId = "intervention_id"
        case skipReason = "skip_reason"
        case loggedAt = "logged_at"
    }
}

// MARK: - Missed Dose Item (GET /protocols/runs/missed-doses)

struct MissedDoseItem: Codable, Sendable, Identifiable {
    var id: String { "\(runId)-\(protocolLineId)-\(dayNumber)" }
    let protocolId: String
    let protocolName: String
    let runId: String
    let protocolLineId: String
    let substance: String
    let dose: Double?
    let unit: String?
    let route: String?
    let timeOfDay: String?
    let dayNumber: Int
    let date: String
    let status: DoseStatus

    enum CodingKeys: String, CodingKey {
        case protocolId = "protocol_id"
        case protocolName = "protocol_name"
        case runId = "run_id"
        case protocolLineId = "protocol_line_id"
        case substance, dose, unit, route
        case timeOfDay = "time_of_day"
        case dayNumber = "day_number"
        case date, status
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
    /// `completed_closed / (scheduled_closed - skipped_closed) * 100`, same
    /// definition as `AdherenceResponse.adherencePct`. Populated on
    /// `GET /protocols/runs/active` and run-creation responses; `nil` on
    /// placeholder-only paths, e.g. the `runs` embedded in
    /// `GET /protocols/:id` (used for the paused-run adherence fallback —
    /// see `ProtocolsViewModel.currentRun(for:)`), which the backend
    /// currently leaves unpopulated. See docs/architecture/api.md.
    let adherencePct: Double?
    let dosesMissed: Int?

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
        case adherencePct = "adherence_pct"
        case dosesMissed = "doses_missed"
    }

    // Explicit memberwise init (with defaults for the two adherence fields)
    // so existing test call sites built before those fields were added keep
    // compiling. Codable's `init(from:)`/`encode(to:)` are still
    // compiler-synthesized since neither is hand-written here.
    init(
        id: String,
        protocolId: String,
        protocolName: String?,
        startDate: String,
        durationDays: Int?,
        status: String,
        notify: Bool,
        notifyTime: String?,
        notifyTimes: [String]?,
        repeatReminders: Bool,
        repeatIntervalMinutes: Int?,
        progressPct: Double,
        dosesToday: Int,
        dosesCompletedToday: Int,
        createdAt: String,
        adherencePct: Double? = nil,
        dosesMissed: Int? = nil
    ) {
        self.id = id
        self.protocolId = protocolId
        self.protocolName = protocolName
        self.startDate = startDate
        self.durationDays = durationDays
        self.status = status
        self.notify = notify
        self.notifyTime = notifyTime
        self.notifyTimes = notifyTimes
        self.repeatReminders = repeatReminders
        self.repeatIntervalMinutes = repeatIntervalMinutes
        self.progressPct = progressPct
        self.dosesToday = dosesToday
        self.dosesCompletedToday = dosesCompletedToday
        self.createdAt = createdAt
        self.adherencePct = adherencePct
        self.dosesMissed = dosesMissed
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
    // The backend's DISTINCT ON explicitly permits rows that differ only by
    // route, so route must be part of the id or two such rows collide in a
    // ForEach (duplicate ids -> duplicate accessibility ids, dropped rows).
    // `dose.map(String.init) ?? "nil"` (not `dose ?? 0`) so a genuinely nil
    // dose can't collide with a genuine 0-dose row either.
    var id: String {
        let doseComponent = dose.map { "\($0)" } ?? "nil"
        return "\(protocolName)-\(substance)-\(doseComponent)-\(unit ?? "")-\(route ?? "")"
    }
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

// MARK: - Today's Doses (attribution parity for quick-pick)

/// One entry per scheduled line across the user's active runs for *today*
/// only. Used to detect when a quick-picked substance+dose matches a
/// still-pending scheduled dose, so `InterventionForm` can offer counting
/// the entry toward the protocol instead of creating a free-floating
/// intervention. Modeled on the backend's `TodaysDoseItem`
/// (`backend/api/src/models/protocol.rs`).
struct TodaysDose: Codable, Sendable, Identifiable {
    var id: String { protocolLineId }
    let protocolId: String
    let protocolName: String
    let runId: String
    let protocolLineId: String
    let substance: String
    let dose: Double?
    let unit: String?
    let route: String?
    let timeOfDay: String?
    let dayNumber: Int
    let status: DoseStatus?

    enum CodingKeys: String, CodingKey {
        case protocolId = "protocol_id"
        case protocolName = "protocol_name"
        case runId = "run_id"
        case protocolLineId = "protocol_line_id"
        case substance, dose, unit, route
        case timeOfDay = "time_of_day"
        case dayNumber = "day_number"
        case status
    }
}

// MARK: - Endpoint Extensions

extension Endpoints {
    static let protocols = "/api/v1/protocols"
    static let activeRuns = "/api/v1/protocols/runs/active"
    static let activeSubstances = "/api/v1/protocols/active-substances"
    static let todaysDoses = "/api/v1/protocols/runs/todays-doses"

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

    static func deleteDose(runId: String, doseId: String) -> String {
        "/api/v1/protocols/runs/\(runId)/doses/\(doseId)"
    }

    static func runDoses(_ runId: String, fromDay: Int? = nil, toDay: Int? = nil) -> String {
        var query: [String] = []
        if let fromDay { query.append("from_day=\(fromDay)") }
        if let toDay { query.append("to_day=\(toDay)") }
        let base = "/api/v1/protocols/runs/\(runId)/doses"
        return query.isEmpty ? base : base + "?" + query.joined(separator: "&")
    }

    static let missedDoses = "/api/v1/protocols/runs/missed-doses"

    static func runAdherence(_ runId: String) -> String {
        "/api/v1/protocols/runs/\(runId)/adherence"
    }
}
