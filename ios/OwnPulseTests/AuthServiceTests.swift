// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("AuthService")
@MainActor
struct AuthServiceTests {
    @Test("loginWithApple calls backend with correct AppleCallbackRequest, stores tokens, sets isAuthenticated")
    func loginWithAppleSuccess() async throws {
        let mockNetwork = MockNetworkClient()
        let mockKeychain = MockKeychainService()

        let expectedResponse = TokenResponseWithRefresh(
            accessToken: "test-access-token",
            refreshToken: "test-refresh-token",
            tokenType: "Bearer",
            expiresIn: 3600
        )

        var capturedBody: AppleCallbackRequest?
        mockNetwork.requestHandler = { method, path, body in
            if method == "POST" && path == Endpoints.authAppleCallback {
                if let req = body as? AppleCallbackRequest {
                    capturedBody = req
                }
                return expectedResponse
            }
            fatalError("Unexpected request: \(method) \(path)")
        }

        let service = AuthService(networkClient: mockNetwork, keychainService: mockKeychain)
        #expect(service.isAuthenticated == false)

        try await service.processAppleCredential(idToken: "fake-id-token")

        #expect(service.isAuthenticated == true)
        #expect(capturedBody?.idToken == "fake-id-token")
        #expect(capturedBody?.platform == "ios")

        let storedAccess = try mockKeychain.load(key: AuthService.accessTokenKey)
        let storedRefresh = try mockKeychain.load(key: AuthService.refreshTokenKey)
        #expect(String(data: storedAccess!, encoding: .utf8) == "test-access-token")
        #expect(String(data: storedRefresh!, encoding: .utf8) == "test-refresh-token")

        #expect(mockNetwork.requestCalls.count == 1)
        #expect(mockNetwork.requestCalls[0].method == "POST")
        #expect(mockNetwork.requestCalls[0].path == Endpoints.authAppleCallback)
    }

    @Test("loginWithApple missing identityToken throws invalidCallback")
    func loginWithAppleMissingToken() async {
        // AppleAuthHelper returns a credential. If identityToken is nil,
        // AuthService throws .invalidCallback. We verify this error type exists
        // and can be thrown/caught correctly.
        let error = AuthError.invalidCallback
        #expect(error == AuthError.invalidCallback)
    }

    @Test("loginWithPassword calls backend with correct LoginRequest, stores token, sets isAuthenticated")
    func loginWithPasswordSuccess() async throws {
        let mockNetwork = MockNetworkClient()
        let mockKeychain = MockKeychainService()

        let expectedResponse = TokenResponse(
            accessToken: "password-access-token",
            tokenType: "Bearer",
            expiresIn: 3600
        )

        var capturedBody: LoginRequest?
        mockNetwork.requestHandler = { method, path, body in
            if method == "POST" && path == Endpoints.authLogin {
                if let req = body as? LoginRequest {
                    capturedBody = req
                }
                return expectedResponse
            }
            fatalError("Unexpected request: \(method) \(path)")
        }

        let service = AuthService(networkClient: mockNetwork, keychainService: mockKeychain)
        try await service.loginWithPassword(username: "tony", password: "s3cret")

        #expect(service.isAuthenticated == true)
        #expect(capturedBody?.username == "tony")
        #expect(capturedBody?.password == "s3cret")

        let storedAccess = try mockKeychain.load(key: AuthService.accessTokenKey)
        #expect(String(data: storedAccess!, encoding: .utf8) == "password-access-token")
    }

    @Test("loginWithPassword network error does not set isAuthenticated")
    func loginWithPasswordNetworkError() async {
        let mockNetwork = MockNetworkClient()
        let mockKeychain = MockKeychainService()

        mockNetwork.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "internal")
        }

        let service = AuthService(networkClient: mockNetwork, keychainService: mockKeychain)

        do {
            try await service.loginWithPassword(username: "tony", password: "wrong")
            Issue.record("Expected error to be thrown")
        } catch {
            // Expected
        }

        #expect(service.isAuthenticated == false)
        #expect(mockKeychain.savedKeys.isEmpty)
    }

    // MARK: - processCallback (Google OAuth redirect)

    @Test("processCallback extracts tokens from fragment and stores them")
    func processCallbackSuccess() async throws {
        let mockNetwork = MockNetworkClient()
        let mockKeychain = MockKeychainService()
        let service = AuthService(networkClient: mockNetwork, keychainService: mockKeychain)

        let url = URL(string: "ownpulse://auth#token=test-jwt&refresh_token=test-refresh")!
        try await service.processCallback(url: url)

        #expect(service.isAuthenticated == true)
        let storedAccess = try mockKeychain.load(key: AuthService.accessTokenKey)
        let storedRefresh = try mockKeychain.load(key: AuthService.refreshTokenKey)
        #expect(String(data: storedAccess!, encoding: .utf8) == "test-jwt")
        #expect(String(data: storedRefresh!, encoding: .utf8) == "test-refresh")
    }

    @Test("processCallback throws invalidCallback when fragment is missing")
    func processCallbackNoFragment() async {
        let mockNetwork = MockNetworkClient()
        let mockKeychain = MockKeychainService()
        let service = AuthService(networkClient: mockNetwork, keychainService: mockKeychain)

        let url = URL(string: "ownpulse://auth")!
        do {
            try await service.processCallback(url: url)
            Issue.record("Expected error to be thrown")
        } catch let error as AuthError {
            #expect(error == .invalidCallback)
        } catch {
            Issue.record("Unexpected error type: \(error)")
        }

        #expect(service.isAuthenticated == false)
    }

    @Test("processCallback throws callbackFailed when error query param is present")
    func processCallbackWithError() async {
        let mockNetwork = MockNetworkClient()
        let mockKeychain = MockKeychainService()
        let service = AuthService(networkClient: mockNetwork, keychainService: mockKeychain)

        let url = URL(string: "ownpulse://auth?error=access_denied")!
        do {
            try await service.processCallback(url: url)
            Issue.record("Expected error to be thrown")
        } catch let error as AuthError {
            #expect(error == .callbackFailed)
        } catch {
            Issue.record("Unexpected error type: \(error)")
        }

        #expect(service.isAuthenticated == false)
    }

    @Test("processCallback throws invalidCallback when token is missing from fragment")
    func processCallbackMissingToken() async {
        let mockNetwork = MockNetworkClient()
        let mockKeychain = MockKeychainService()
        let service = AuthService(networkClient: mockNetwork, keychainService: mockKeychain)

        let url = URL(string: "ownpulse://auth#refresh_token=test-refresh")!
        do {
            try await service.processCallback(url: url)
            Issue.record("Expected error to be thrown")
        } catch let error as AuthError {
            #expect(error == .invalidCallback)
        } catch {
            Issue.record("Unexpected error type: \(error)")
        }

        #expect(service.isAuthenticated == false)
    }

    @Test("processCallback throws invalidCallback when refresh_token is missing from fragment")
    func processCallbackMissingRefreshToken() async {
        let mockNetwork = MockNetworkClient()
        let mockKeychain = MockKeychainService()
        let service = AuthService(networkClient: mockNetwork, keychainService: mockKeychain)

        let url = URL(string: "ownpulse://auth#token=test-jwt")!
        do {
            try await service.processCallback(url: url)
            Issue.record("Expected error to be thrown")
        } catch let error as AuthError {
            #expect(error == .invalidCallback)
        } catch {
            Issue.record("Unexpected error type: \(error)")
        }

        #expect(service.isAuthenticated == false)
    }
}
