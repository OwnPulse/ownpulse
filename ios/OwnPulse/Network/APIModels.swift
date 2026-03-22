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

struct HealthKitWriteQueueItem: Codable, Sendable {
    let id: String
    let hkType: String
    let value: Double
    let scheduledAt: Date

    enum CodingKeys: String, CodingKey {
        case id
        case hkType = "hk_type"
        case value
        case scheduledAt = "scheduled_at"
    }
}

struct HealthKitConfirm: Codable, Sendable {
    let ids: [String]
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
