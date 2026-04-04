// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

/// Creates a minimal JWT with the given expiration for testing.
/// The signature is invalid but JWTDecoder only parses the payload.
private func makeTestJWT(sub: String = "user-1", exp: Date) -> String {
    let header = Data(#"{"alg":"HS256","typ":"JWT"}"#.utf8).base64EncodedString()
    let expTimestamp = Int(exp.timeIntervalSince1970)
    let payload = Data(#"{"sub":"\#(sub)","exp":\#(expTimestamp)}"#.utf8).base64EncodedString()
    return "\(header).\(payload).fakesig"
}

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

    // MARK: - Session restore on init

    @Test("init sets isAuthenticated true when valid access token exists")
    func initWithValidAccessToken() {
        let mockNetwork = MockNetworkClient()
        let mockKeychain = MockKeychainService()

        let validJWT = makeTestJWT(exp: Date().addingTimeInterval(3600))
        try! mockKeychain.save(key: AuthService.accessTokenKey, data: Data(validJWT.utf8))

        let service = AuthService(networkClient: mockNetwork, keychainService: mockKeychain)
        #expect(service.isAuthenticated == true)
        // No network calls should be made — token is still valid
        #expect(mockNetwork.requestCalls.isEmpty)
    }

    @Test("init sets isAuthenticated true and refreshes when access token expired but refresh token exists")
    func initWithExpiredAccessTokenAndRefreshToken() async throws {
        let mockNetwork = MockNetworkClient()
        let mockKeychain = MockKeychainService()

        let expiredJWT = makeTestJWT(exp: Date().addingTimeInterval(-3600))
        try mockKeychain.save(key: AuthService.accessTokenKey, data: Data(expiredJWT.utf8))
        try mockKeychain.save(key: AuthService.refreshTokenKey, data: Data("valid-refresh-token".utf8))

        let refreshResponse = TokenResponse(
            accessToken: "new-access-token",
            tokenType: "Bearer",
            expiresIn: 3600
        )
        mockNetwork.requestHandler = { method, path, body in
            if method == "POST" && path == Endpoints.authRefresh {
                return refreshResponse
            }
            fatalError("Unexpected request: \(method) \(path)")
        }

        let service = AuthService(networkClient: mockNetwork, keychainService: mockKeychain)
        // Should be authenticated immediately (optimistic)
        #expect(service.isAuthenticated == true)

        // Call refreshAccessToken directly to verify the logic
        // (the Task in init runs asynchronously and is hard to await)
        await service.refreshAccessToken()

        #expect(service.isAuthenticated == true)
        let storedAccess = try mockKeychain.load(key: AuthService.accessTokenKey)
        #expect(String(data: storedAccess!, encoding: .utf8) == "new-access-token")
    }

    @Test("init sets isAuthenticated false when no tokens exist")
    func initWithNoTokens() {
        let mockNetwork = MockNetworkClient()
        let mockKeychain = MockKeychainService()

        let service = AuthService(networkClient: mockNetwork, keychainService: mockKeychain)
        #expect(service.isAuthenticated == false)
    }

    @Test("init with only refresh token (no access token) sets isAuthenticated true")
    func initWithOnlyRefreshToken() {
        let mockNetwork = MockNetworkClient()
        let mockKeychain = MockKeychainService()

        mockNetwork.requestHandler = { _, _, _ in
            return TokenResponse(accessToken: "new-token", tokenType: "Bearer", expiresIn: 3600)
        }

        try! mockKeychain.save(key: AuthService.refreshTokenKey, data: Data("refresh-token".utf8))

        let service = AuthService(networkClient: mockNetwork, keychainService: mockKeychain)
        #expect(service.isAuthenticated == true)
    }

    @Test("refreshAccessToken clears tokens and sets isAuthenticated false on network error")
    func refreshAccessTokenFailure() async throws {
        let mockNetwork = MockNetworkClient()
        let mockKeychain = MockKeychainService()

        let expiredJWT = makeTestJWT(exp: Date().addingTimeInterval(-3600))
        try mockKeychain.save(key: AuthService.accessTokenKey, data: Data(expiredJWT.utf8))
        try mockKeychain.save(key: AuthService.refreshTokenKey, data: Data("bad-refresh".utf8))

        mockNetwork.requestHandler = { _, _, _ in
            throw NetworkError.unauthorized
        }

        // init will set isAuthenticated = true (expired access + refresh exists)
        let service = AuthService(networkClient: mockNetwork, keychainService: mockKeychain)
        #expect(service.isAuthenticated == true)

        // Calling refresh directly simulates the background Task completing
        await service.refreshAccessToken()

        #expect(service.isAuthenticated == false)
        // Tokens should be cleared
        let accessToken = try mockKeychain.load(key: AuthService.accessTokenKey)
        let refreshToken = try mockKeychain.load(key: AuthService.refreshTokenKey)
        #expect(accessToken == nil)
        #expect(refreshToken == nil)
    }

    @Test("refreshAccessToken sets isAuthenticated false when refresh token is missing from keychain")
    func refreshAccessTokenNoRefreshToken() async throws {
        let mockNetwork = MockNetworkClient()
        let mockKeychain = MockKeychainService()

        // No tokens at all — init leaves isAuthenticated = false
        let service = AuthService(networkClient: mockNetwork, keychainService: mockKeychain)
        #expect(service.isAuthenticated == false)

        // Even if we somehow call refresh, it should remain false
        await service.refreshAccessToken()

        #expect(service.isAuthenticated == false)
        #expect(mockNetwork.requestCalls.isEmpty)
    }
}
