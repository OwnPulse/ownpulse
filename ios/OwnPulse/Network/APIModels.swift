// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

struct TokenResponse: Codable, Sendable {
    let accessToken: String
    let tokenType: String
    let expiresIn: Int

    enum CodingKeys: String, CodingKey {
        case accessToken = "access_token"
        case tokenType = "token_type"
        case expiresIn = "expires_in"
    }
}

struct AuthCallbackResponse: Codable, Sendable {
    let token: String
    let refreshToken: String

    enum CodingKeys: String, CodingKey {
        case token
        case refreshToken = "refresh_token"
    }
}

struct RefreshRequest: Codable, Sendable {
    let refreshToken: String

    enum CodingKeys: String, CodingKey {
        case refreshToken = "refresh_token"
    }
}

struct CreateHealthRecord: Codable, Sendable {
    let source: String
    let recordType: String
    let value: Double
    let unit: String
    let startTime: Date
    let endTime: Date
    let metadata: [String: String]?
    let sourceId: String?

    enum CodingKeys: String, CodingKey {
        case source
        case recordType = "record_type"
        case value, unit
        case startTime = "start_time"
        case endTime = "end_time"
        case metadata
        case sourceId = "source_id"
    }
}

struct HealthKitBulkInsert: Codable, Sendable {
    let records: [CreateHealthRecord]
}

struct HealthRecordResponse: Codable, Sendable {
    let id: String
    let userId: String
    let source: String
    let recordType: String
    let value: Double
    let unit: String
    let startTime: Date
    let endTime: Date

    enum CodingKeys: String, CodingKey {
        case id
        case userId = "user_id"
        case source
        case recordType = "record_type"
        case value, unit
        case startTime = "start_time"
        case endTime = "end_time"
    }
}

/// The write-queue item's `value` is not a bare number — the backend stores
/// the full sample shape it needs to reconstruct on write-back (the queued
/// scheduling timestamp and the sample's own start/end and unit can differ).
/// Pinned by the `ios-backend.json` pact contract; see `HealthKitWriteQueueDecodeTests`.
struct HealthKitWriteQueuePayload: Codable, Sendable {
    /// Optional: the backend's `HealthRecordRow` has no route-level
    /// validation requiring a numeric value, so a record enqueued without
    /// one serves `null` here. A quantity sample can't be written to
    /// HealthKit without a value — callers must treat `nil` as a failure,
    /// never as 0 or any other placeholder.
    let value: Double?
    let unit: String?
    /// Always present — every write-queue row has a start time.
    let startTime: Date
    let endTime: Date?

    enum CodingKeys: String, CodingKey {
        case value, unit
        case startTime = "start_time"
        case endTime = "end_time"
    }
}

struct HealthKitWriteQueueItem: Codable, Sendable, Identifiable {
    let id: String
    let userId: String?
    let hkType: String
    let value: HealthKitWriteQueuePayload
    let scheduledAt: Date
    let confirmedAt: Date?
    let failedAt: Date?
    let error: String?
    let sourceRecordId: String?
    let sourceTable: String?

    enum CodingKeys: String, CodingKey {
        case id
        case userId = "user_id"
        case hkType = "hk_type"
        case value
        case scheduledAt = "scheduled_at"
        case confirmedAt = "confirmed_at"
        case failedAt = "failed_at"
        case error
        case sourceRecordId = "source_record_id"
        case sourceTable = "source_table"
    }
}

struct HealthKitConfirmFailure: Codable, Sendable {
    let id: String
    let error: String
}

struct HealthKitConfirm: Codable, Sendable {
    let ids: [String]
    let failures: [HealthKitConfirmFailure]

    init(ids: [String], failures: [HealthKitConfirmFailure] = []) {
        self.ids = ids
        self.failures = failures
    }
}

struct AppleCallbackRequest: Codable, Sendable {
    let idToken: String
    let platform: String

    enum CodingKeys: String, CodingKey {
        case idToken = "id_token"
        case platform
    }
}

struct LoginRequest: Codable, Sendable {
    let username: String
    let password: String
}

struct TokenResponseWithRefresh: Codable, Sendable {
    let accessToken: String
    let refreshToken: String
    let tokenType: String
    let expiresIn: Int

    enum CodingKeys: String, CodingKey {
        case accessToken = "access_token"
        case refreshToken = "refresh_token"
        case tokenType = "token_type"
        case expiresIn = "expires_in"
    }
}

struct AuthMethod: Codable, Sendable, Identifiable {
    let id: String
    let provider: String
    let email: String?
    let createdAt: Date

    enum CodingKeys: String, CodingKey {
        case id, provider, email
        case createdAt = "created_at"
    }
}

struct LinkAuthRequest: Codable, Sendable {
    let provider: String
    let idToken: String?
    let password: String?

    enum CodingKeys: String, CodingKey {
        case provider
        case idToken = "id_token"
        case password
    }
}

struct CreateLabResultRecord: Codable, Sendable {
    let panelDate: String
    let labName: String?
    let marker: String
    let value: Double
    let unit: String
    let referenceLow: Double?
    let referenceHigh: Double?
    let source: String
    let sourceId: String?

    enum CodingKeys: String, CodingKey {
        case panelDate = "panel_date"
        case labName = "lab_name"
        case marker, value, unit
        case referenceLow = "reference_low"
        case referenceHigh = "reference_high"
        case source
        case sourceId = "source_id"
    }
}

struct BulkCreateLabResults: Codable, Sendable {
    let records: [CreateLabResultRecord]
}

struct LabResultResponse: Codable, Sendable {
    let id: String
}
