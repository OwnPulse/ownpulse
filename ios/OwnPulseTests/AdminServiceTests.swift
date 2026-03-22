// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

// MARK: - Mock for Admin tests (throws instead of fatalError for missing handler)

@MainActor
private final class AdminMockNetworkClient: NetworkClientProtocol, @unchecked Sendable {
    var requestHandler: ((String, String, (any Encodable & Sendable)?) throws -> Any)?
    var requestNoContentHandler: ((String, String, (any Encodable & Sendable)?) throws -> Void)?

    func request<T: Decodable & Sendable>(
        method: String,
        path: String,
        body: (any Encodable & Sendable)?
    ) async throws -> T {
        guard let handler = requestHandler else {
            throw NetworkError.noData
        }
        guard let result = try handler(method, path, body) as? T else {
            throw NetworkError.noData
        }
        return result
    }

    func requestNoContent(
        method: String,
        path: String,
        body: (any Encodable & Sendable)?
    ) async throws {
        guard let handler = requestNoContentHandler else {
            throw NetworkError.noData
        }
        try handler(method, path, body)
    }
}

// MARK: - AdminService Tests

@Suite("AdminService", .serialized)
@MainActor
struct AdminServiceTests {
    private let mockClient = AdminMockNetworkClient()

    @Test("listUsers calls GET on admin users endpoint")
    func listUsers() async throws {
        let expectedUsers = [
            AdminUser(
                id: "user-1",
                username: "alice",
                authProvider: "google",
                email: "alice@example.com",
                role: "admin",
                status: "active",
                dataRegion: "us",
                createdAt: Date(timeIntervalSince1970: 1_700_000_000)
            ),
        ]

        mockClient.requestHandler = { method, path, _ in
            #expect(method == "GET")
            #expect(path == Endpoints.adminUsers)
            return expectedUsers
        }

        let service = AdminService(networkClient: mockClient)
        let users = try await service.listUsers()
        #expect(users.count == 1)
        #expect(users[0].username == "alice")
    }

    @Test("updateRole calls PATCH with role body")
    func updateRole() async throws {
        let expectedUser = AdminUser(
            id: "user-1",
            username: "alice",
            authProvider: "google",
            email: nil,
            role: "user",
            status: "active",
            dataRegion: "us",
            createdAt: Date(timeIntervalSince1970: 1_700_000_000)
        )

        mockClient.requestHandler = { method, path, body in
            #expect(method == "PATCH")
            #expect(path == "\(Endpoints.adminUsers)/user-1/role")
            return expectedUser
        }

        let service = AdminService(networkClient: mockClient)
        let updated = try await service.updateRole(userId: "user-1", role: "user")
        #expect(updated.role == "user")
    }

    @Test("updateStatus calls PATCH with status body")
    func updateStatus() async throws {
        let expectedUser = AdminUser(
            id: "user-1",
            username: "alice",
            authProvider: "google",
            email: nil,
            role: "admin",
            status: "disabled",
            dataRegion: "us",
            createdAt: Date(timeIntervalSince1970: 1_700_000_000)
        )

        mockClient.requestHandler = { method, path, _ in
            #expect(method == "PATCH")
            #expect(path == "\(Endpoints.adminUsers)/user-1/status")
            return expectedUser
        }

        let service = AdminService(networkClient: mockClient)
        let updated = try await service.updateStatus(userId: "user-1", status: "disabled")
        #expect(updated.status == "disabled")
    }

    @Test("deleteUser calls DELETE on user endpoint")
    func deleteUser() async throws {
        var called = false
        mockClient.requestNoContentHandler = { method, path, _ in
            #expect(method == "DELETE")
            #expect(path == "\(Endpoints.adminUsers)/user-1")
            called = true
        }

        let service = AdminService(networkClient: mockClient)
        try await service.deleteUser(userId: "user-1")
        #expect(called)
    }

    @Test("listInvites calls GET on admin invites endpoint")
    func listInvites() async throws {
        let expectedInvites = [
            InviteCode(
                id: "inv-1",
                code: "ABC123",
                label: "Friends",
                maxUses: 10,
                useCount: 3,
                expiresAt: nil,
                revokedAt: nil,
                createdAt: Date(timeIntervalSince1970: 1_700_000_000)
            ),
        ]

        mockClient.requestHandler = { method, path, _ in
            #expect(method == "GET")
            #expect(path == Endpoints.adminInvites)
            return expectedInvites
        }

        let service = AdminService(networkClient: mockClient)
        let invites = try await service.listInvites()
        #expect(invites.count == 1)
        #expect(invites[0].code == "ABC123")
    }

    @Test("createInvite calls POST with request body")
    func createInvite() async throws {
        let expectedInvite = InviteCode(
            id: "inv-2",
            code: "XYZ789",
            label: "Team",
            maxUses: 5,
            useCount: 0,
            expiresAt: nil,
            revokedAt: nil,
            createdAt: Date(timeIntervalSince1970: 1_700_000_000)
        )

        mockClient.requestHandler = { method, path, _ in
            #expect(method == "POST")
            #expect(path == Endpoints.adminInvites)
            return expectedInvite
        }

        let service = AdminService(networkClient: mockClient)
        let invite = try await service.createInvite(
            label: "Team", maxUses: 5, expiresInHours: 24
        )
        #expect(invite.code == "XYZ789")
        #expect(invite.maxUses == 5)
    }

    @Test("revokeInvite calls DELETE on invite endpoint")
    func revokeInvite() async throws {
        let revokedInvite = InviteCode(
            id: "inv-1",
            code: "ABC123",
            label: nil,
            maxUses: nil,
            useCount: 0,
            expiresAt: nil,
            revokedAt: Date(),
            createdAt: Date(timeIntervalSince1970: 1_700_000_000)
        )

        mockClient.requestHandler = { method, path, _ in
            #expect(method == "DELETE")
            #expect(path == "\(Endpoints.adminInvites)/inv-1")
            return revokedInvite
        }

        let service = AdminService(networkClient: mockClient)
        let result = try await service.revokeInvite(id: "inv-1")
        #expect(result.revokedAt != nil)
    }
}

// MARK: - AdminService Error Path Tests

@Suite("AdminService error paths", .serialized)
@MainActor
struct AdminServiceErrorTests {
    private let mockClient = AdminMockNetworkClient()

    // MARK: - Network failure tests

    @Test("listUsers propagates network error")
    func listUsersNetworkError() async {
        mockClient.requestHandler = { _, _, _ in
            throw URLError(.notConnectedToInternet)
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: URLError.self) {
            _ = try await service.listUsers()
        }
    }

    @Test("updateRole propagates network error")
    func updateRoleNetworkError() async {
        mockClient.requestHandler = { _, _, _ in
            throw URLError(.timedOut)
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: URLError.self) {
            _ = try await service.updateRole(userId: "user-1", role: "user")
        }
    }

    @Test("updateStatus propagates network error")
    func updateStatusNetworkError() async {
        mockClient.requestHandler = { _, _, _ in
            throw URLError(.networkConnectionLost)
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: URLError.self) {
            _ = try await service.updateStatus(userId: "user-1", status: "disabled")
        }
    }

    @Test("deleteUser propagates network error")
    func deleteUserNetworkError() async {
        mockClient.requestNoContentHandler = { _, _, _ in
            throw URLError(.notConnectedToInternet)
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: URLError.self) {
            try await service.deleteUser(userId: "user-1")
        }
    }

    @Test("listInvites propagates network error")
    func listInvitesNetworkError() async {
        mockClient.requestHandler = { _, _, _ in
            throw URLError(.cannotFindHost)
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: URLError.self) {
            _ = try await service.listInvites()
        }
    }

    @Test("createInvite propagates network error")
    func createInviteNetworkError() async {
        mockClient.requestHandler = { _, _, _ in
            throw URLError(.timedOut)
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: URLError.self) {
            _ = try await service.createInvite(
                label: "Team", maxUses: 5, expiresInHours: 24
            )
        }
    }

    @Test("revokeInvite propagates network error")
    func revokeInviteNetworkError() async {
        mockClient.requestHandler = { _, _, _ in
            throw URLError(.networkConnectionLost)
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: URLError.self) {
            _ = try await service.revokeInvite(id: "inv-1")
        }
    }

    // MARK: - 401 Unauthorized tests

    @Test("listUsers throws on 401 unauthorized")
    func listUsersUnauthorized() async {
        mockClient.requestHandler = { _, _, _ in
            throw NetworkError.unauthorized
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: NetworkError.self) {
            _ = try await service.listUsers()
        }
    }

    @Test("updateRole throws on 401 unauthorized")
    func updateRoleUnauthorized() async {
        mockClient.requestHandler = { _, _, _ in
            throw NetworkError.unauthorized
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: NetworkError.self) {
            _ = try await service.updateRole(userId: "user-1", role: "admin")
        }
    }

    @Test("updateStatus throws on 401 unauthorized")
    func updateStatusUnauthorized() async {
        mockClient.requestHandler = { _, _, _ in
            throw NetworkError.unauthorized
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: NetworkError.self) {
            _ = try await service.updateStatus(userId: "user-1", status: "active")
        }
    }

    @Test("deleteUser throws on 401 unauthorized")
    func deleteUserUnauthorized() async {
        mockClient.requestNoContentHandler = { _, _, _ in
            throw NetworkError.unauthorized
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: NetworkError.self) {
            try await service.deleteUser(userId: "user-1")
        }
    }

    @Test("listInvites throws on 401 unauthorized")
    func listInvitesUnauthorized() async {
        mockClient.requestHandler = { _, _, _ in
            throw NetworkError.unauthorized
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: NetworkError.self) {
            _ = try await service.listInvites()
        }
    }

    @Test("createInvite throws on 401 unauthorized")
    func createInviteUnauthorized() async {
        mockClient.requestHandler = { _, _, _ in
            throw NetworkError.unauthorized
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: NetworkError.self) {
            _ = try await service.createInvite(
                label: nil, maxUses: nil, expiresInHours: nil
            )
        }
    }

    @Test("revokeInvite throws on 401 unauthorized")
    func revokeInviteUnauthorized() async {
        mockClient.requestHandler = { _, _, _ in
            throw NetworkError.unauthorized
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: NetworkError.self) {
            _ = try await service.revokeInvite(id: "inv-1")
        }
    }

    // MARK: - 403 Forbidden tests

    @Test("listUsers throws on 403 forbidden")
    func listUsersForbidden() async {
        mockClient.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 403, body: "Forbidden")
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: NetworkError.self) {
            _ = try await service.listUsers()
        }
    }

    @Test("updateRole throws on 403 forbidden")
    func updateRoleForbidden() async {
        mockClient.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 403, body: "Forbidden")
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: NetworkError.self) {
            _ = try await service.updateRole(userId: "user-1", role: "admin")
        }
    }

    @Test("updateStatus throws on 403 forbidden")
    func updateStatusForbidden() async {
        mockClient.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 403, body: "Forbidden")
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: NetworkError.self) {
            _ = try await service.updateStatus(userId: "user-1", status: "active")
        }
    }

    @Test("deleteUser throws on 403 forbidden")
    func deleteUserForbidden() async {
        mockClient.requestNoContentHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 403, body: "Forbidden")
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: NetworkError.self) {
            try await service.deleteUser(userId: "user-1")
        }
    }

    @Test("listInvites throws on 403 forbidden")
    func listInvitesForbidden() async {
        mockClient.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 403, body: "Forbidden")
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: NetworkError.self) {
            _ = try await service.listInvites()
        }
    }

    @Test("createInvite throws on 403 forbidden")
    func createInviteForbidden() async {
        mockClient.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 403, body: "Forbidden")
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: NetworkError.self) {
            _ = try await service.createInvite(
                label: "Team", maxUses: 5, expiresInHours: 24
            )
        }
    }

    @Test("revokeInvite throws on 403 forbidden")
    func revokeInviteForbidden() async {
        mockClient.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 403, body: "Forbidden")
        }

        let service = AdminService(networkClient: mockClient)
        await #expect(throws: NetworkError.self) {
            _ = try await service.revokeInvite(id: "inv-1")
        }
    }
}

// MARK: - InviteCode.isActive Tests

@Suite("InviteCode.isActive")
struct InviteCodeIsActiveTests {
    @Test("active invite with no expiry and not revoked")
    func activeNoExpiry() {
        let invite = InviteCode(
            id: "1", code: "A", label: nil, maxUses: nil,
            useCount: 0, expiresAt: nil, revokedAt: nil,
            createdAt: Date()
        )
        #expect(invite.isActive == true)
    }

    @Test("revoked invite is not active")
    func revokedNotActive() {
        let invite = InviteCode(
            id: "1", code: "A", label: nil, maxUses: nil,
            useCount: 0, expiresAt: nil, revokedAt: Date(),
            createdAt: Date()
        )
        #expect(invite.isActive == false)
    }

    @Test("expired invite is not active")
    func expiredNotActive() {
        let invite = InviteCode(
            id: "1", code: "A", label: nil, maxUses: nil,
            useCount: 0,
            expiresAt: Date(timeIntervalSinceNow: -3600),
            revokedAt: nil,
            createdAt: Date()
        )
        #expect(invite.isActive == false)
    }

    @Test("invite with future expiry is active")
    func futureExpiryIsActive() {
        let invite = InviteCode(
            id: "1", code: "A", label: nil, maxUses: nil,
            useCount: 0,
            expiresAt: Date(timeIntervalSinceNow: 3600),
            revokedAt: nil,
            createdAt: Date()
        )
        #expect(invite.isActive == true)
    }
}

// MARK: - JWTDecoder role Tests

@Suite("JWTDecoder role parsing")
struct JWTDecoderRoleTests {
    @Test("decodes role from JWT payload")
    func decodesRole() {
        // Payload: {"sub":"user-1","exp":9999999999,"role":"admin"}
        // Base64URL of that payload:
        // {"sub":"user-1","exp":9999999999,"role":"admin"}
        let payloadJSON = #"{"sub":"user-1","exp":9999999999,"role":"admin"}"#
        let payloadBase64 = Data(payloadJSON.utf8).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")

        let token = "eyJhbGciOiJIUzI1NiJ9.\(payloadBase64).signature"
        let payload = JWTDecoder.decode(token)
        #expect(payload != nil)
        #expect(payload?.role == "admin")
        #expect(payload?.sub == "user-1")
    }

    @Test("role is nil when not present in JWT")
    func roleNilWhenMissing() {
        // Payload: {"sub":"user-1","exp":9999999999}
        let payloadJSON = #"{"sub":"user-1","exp":9999999999}"#
        let payloadBase64 = Data(payloadJSON.utf8).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")

        let token = "eyJhbGciOiJIUzI1NiJ9.\(payloadBase64).signature"
        let payload = JWTDecoder.decode(token)
        #expect(payload != nil)
        #expect(payload?.role == nil)
    }

    @Test("decodes user role from JWT payload")
    func decodesUserRole() {
        let payloadJSON = #"{"sub":"user-2","exp":9999999999,"role":"user"}"#
        let payloadBase64 = Data(payloadJSON.utf8).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")

        let token = "eyJhbGciOiJIUzI1NiJ9.\(payloadBase64).signature"
        let payload = JWTDecoder.decode(token)
        #expect(payload?.role == "user")
    }
}
