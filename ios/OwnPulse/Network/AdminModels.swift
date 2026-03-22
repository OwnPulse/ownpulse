// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

struct AdminUser: Codable, Sendable, Identifiable {
    let id: String
    let username: String
    let authProvider: String
    let email: String?
    let role: String
    let status: String
    let dataRegion: String
    let createdAt: Date

    enum CodingKeys: String, CodingKey {
        case id, username, email, role, status
        case authProvider = "auth_provider"
        case dataRegion = "data_region"
        case createdAt = "created_at"
    }
}

struct InviteCode: Codable, Sendable, Identifiable {
    let id: String
    let code: String
    let label: String?
    let maxUses: Int?
    let useCount: Int
    let expiresAt: Date?
    let revokedAt: Date?
    let createdAt: Date

    enum CodingKeys: String, CodingKey {
        case id, code, label
        case maxUses = "max_uses"
        case useCount = "use_count"
        case expiresAt = "expires_at"
        case revokedAt = "revoked_at"
        case createdAt = "created_at"
    }

    var isActive: Bool {
        revokedAt == nil && (expiresAt == nil || expiresAt! > Date())
    }
}

struct CreateInviteRequest: Codable, Sendable {
    let label: String?
    let maxUses: Int?
    let expiresInHours: Int?

    enum CodingKeys: String, CodingKey {
        case label
        case maxUses = "max_uses"
        case expiresInHours = "expires_in_hours"
    }
}

struct UpdateRoleRequest: Codable, Sendable {
    let role: String
}

struct UpdateStatusRequest: Codable, Sendable {
    let status: String
}
