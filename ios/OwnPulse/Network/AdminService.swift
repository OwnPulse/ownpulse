// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

@Observable
final class AdminService: Sendable {
    private let networkClient: NetworkClientProtocol

    init(networkClient: NetworkClientProtocol) {
        self.networkClient = networkClient
    }

    func listUsers() async throws -> [AdminUser] {
        try await networkClient.request(
            method: "GET",
            path: Endpoints.adminUsers,
            body: nil as String?
        )
    }

    func updateRole(userId: String, role: String) async throws -> AdminUser {
        try await networkClient.request(
            method: "PATCH",
            path: "\(Endpoints.adminUsers)/\(userId)/role",
            body: UpdateRoleRequest(role: role)
        )
    }

    func updateStatus(userId: String, status: String) async throws -> AdminUser {
        try await networkClient.request(
            method: "PATCH",
            path: "\(Endpoints.adminUsers)/\(userId)/status",
            body: UpdateStatusRequest(status: status)
        )
    }

    func deleteUser(userId: String) async throws {
        try await networkClient.requestNoContent(
            method: "DELETE",
            path: "\(Endpoints.adminUsers)/\(userId)",
            body: nil as String?
        )
    }

    func listInvites() async throws -> [InviteCode] {
        try await networkClient.request(
            method: "GET",
            path: Endpoints.adminInvites,
            body: nil as String?
        )
    }

    func createInvite(
        label: String?,
        maxUses: Int?,
        expiresInHours: Int?
    ) async throws -> InviteCode {
        try await networkClient.request(
            method: "POST",
            path: Endpoints.adminInvites,
            body: CreateInviteRequest(
                label: label,
                maxUses: maxUses,
                expiresInHours: expiresInHours
            )
        )
    }

    func revokeInvite(id: String) async throws -> InviteCode {
        try await networkClient.request(
            method: "DELETE",
            path: "\(Endpoints.adminInvites)/\(id)",
            body: nil as String?
        )
    }
}
