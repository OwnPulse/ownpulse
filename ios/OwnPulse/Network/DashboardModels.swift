// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

// MARK: - Dashboard Summary

struct DashboardSummary: Codable, Sendable {
    let latestCheckin: LatestCheckin?
    let checkinCount7d: Int
    let healthRecordCount7d: Int
    let interventionCount7d: Int
    let observationCount7d: Int
    let latestLabDate: String?
    let pendingFriendShares: Int

    enum CodingKeys: String, CodingKey {
        case latestCheckin = "latest_checkin"
        case checkinCount7d = "checkin_count_7d"
        case healthRecordCount7d = "health_record_count_7d"
        case interventionCount7d = "intervention_count_7d"
        case observationCount7d = "observation_count_7d"
        case latestLabDate = "latest_lab_date"
        case pendingFriendShares = "pending_friend_shares"
    }
}

struct LatestCheckin: Codable, Sendable {
    let energy: Int?
    let mood: Int?
    let focus: Int?
    let recovery: Int?
    let libido: Int?
    let date: String

    var isToday: Bool {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withFullDate]
        formatter.timeZone = .current
        guard let checkinDate = formatter.date(from: String(date.prefix(10))) else {
            return false
        }
        return Calendar.current.isDateInToday(checkinDate)
    }
}

// MARK: - Batch Series (Sparklines)

struct BatchSeriesRequest: Codable, Sendable {
    let metrics: [MetricSpec]
    let start: String
    let end: String
    let resolution: String
}

struct MetricSpec: Codable, Sendable {
    let source: String
    let field: String
}

struct BatchSeriesResponse: Codable, Sendable {
    let series: [SeriesData]
}

struct SeriesData: Codable, Sendable, Identifiable {
    let source: String
    let field: String
    let unit: String
    let points: [DataPoint]

    var id: String { "\(source).\(field)" }
}

struct DataPoint: Codable, Sendable {
    let t: String
    let v: Double
    let n: Int
}

// MARK: - Insights

struct Insight: Codable, Sendable, Identifiable {
    let id: String
    let insightType: String
    let headline: String
    let detail: String?
    let createdAt: String

    enum CodingKeys: String, CodingKey {
        case id
        case insightType = "insight_type"
        case headline
        case detail
        case createdAt = "created_at"
    }
}

// MARK: - Data Entry

struct UpsertCheckin: Codable, Sendable {
    let date: String
    let energy: Int
    let mood: Int
    let focus: Int
    let recovery: Int
    let libido: Int
    let notes: String?
}

struct CheckinResponse: Codable, Sendable {
    let id: String
    let date: String
    let energy: Int?
    let mood: Int?
    let focus: Int?
    let recovery: Int?
    let libido: Int?
}

struct CreateIntervention: Codable, Sendable {
    let substance: String
    let dose: Double
    let unit: String
    let route: String
    let administeredAt: String
    let fasted: Bool
    let notes: String?

    enum CodingKeys: String, CodingKey {
        case substance, dose, unit, route
        case administeredAt = "administered_at"
        case fasted, notes
    }
}

struct InterventionResponse: Codable, Sendable {
    let id: String
    let substance: String
}

struct CreateObservation: Codable, Sendable {
    let type: String
    let name: String
    let startTime: String
    let endTime: String?
    let value: [String: AnyCodableValue]

    enum CodingKeys: String, CodingKey {
        case type, name
        case startTime = "start_time"
        case endTime = "end_time"
        case value
    }
}

struct ObservationResponse: Codable, Sendable {
    let id: String
    let type: String
    let name: String
}

// MARK: - AnyCodableValue for JSONB value field

enum AnyCodableValue: Codable, Sendable {
    case string(String)
    case int(Int)
    case double(Double)
    case bool(Bool)

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let v = try? container.decode(Int.self) { self = .int(v); return }
        if let v = try? container.decode(Double.self) { self = .double(v); return }
        if let v = try? container.decode(Bool.self) { self = .bool(v); return }
        if let v = try? container.decode(String.self) { self = .string(v); return }
        throw DecodingError.typeMismatch(
            AnyCodableValue.self,
            .init(codingPath: decoder.codingPath, debugDescription: "Unsupported type")
        )
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let v): try container.encode(v)
        case .int(let v): try container.encode(v)
        case .double(let v): try container.encode(v)
        case .bool(let v): try container.encode(v)
        }
    }
}

// MARK: - Endpoint Extensions

extension Endpoints {
    static let dashboardSummary = "/api/v1/dashboard/summary"
    static let batchSeries = "/api/v1/explore/batch-series"
    static let insights = "/api/v1/insights"
    static let checkins = "/api/v1/checkins"
    static let interventions = "/api/v1/interventions"
    static let observations = "/api/v1/observations"
}
