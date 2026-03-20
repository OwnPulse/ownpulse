// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

/// A sleep record as returned by the backend API.
struct SleepRecord: Codable, Identifiable {
    let id: String
    let userId: String
    let date: String
    let sleepStart: Date?
    let sleepEnd: Date?
    let durationMinutes: Int
    let deepMinutes: Int?
    let lightMinutes: Int?
    let remMinutes: Int?
    let awakeMinutes: Int?
    let score: Int?
    let source: String
    let sourceId: String?
    let notes: String?
    let createdAt: Date

    enum CodingKeys: String, CodingKey {
        case id
        case userId = "user_id"
        case date
        case sleepStart = "sleep_start"
        case sleepEnd = "sleep_end"
        case durationMinutes = "duration_minutes"
        case deepMinutes = "deep_minutes"
        case lightMinutes = "light_minutes"
        case remMinutes = "rem_minutes"
        case awakeMinutes = "awake_minutes"
        case score
        case source
        case sourceId = "source_id"
        case notes
        case createdAt = "created_at"
    }
}

/// The POST body for creating a new sleep record.
struct CreateSleep: Encodable {
    let date: String
    let sleepStart: Date?
    let sleepEnd: Date?
    let durationMinutes: Int
    let deepMinutes: Int?
    let lightMinutes: Int?
    let remMinutes: Int?
    let awakeMinutes: Int?
    let score: Int?
    let source: String
    let sourceId: String?
    let notes: String?

    enum CodingKeys: String, CodingKey {
        case date
        case sleepStart = "sleep_start"
        case sleepEnd = "sleep_end"
        case durationMinutes = "duration_minutes"
        case deepMinutes = "deep_minutes"
        case lightMinutes = "light_minutes"
        case remMinutes = "rem_minutes"
        case awakeMinutes = "awake_minutes"
        case score
        case source
        case sourceId = "source_id"
        case notes
    }
}
