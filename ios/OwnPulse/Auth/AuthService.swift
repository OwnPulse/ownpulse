// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import AuthenticationServices
import Foundation
import Observation
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "auth")

@MainActor
protocol AuthServiceProtocol: Sendable {
    var isAuthenticated: Bool { get }
    func loginWithGoogle() async throws
    func loginWithApple() async throws
    func loginWithPassword(username: String, password: String) async throws
    func logout() async
    func handleCallback(url: URL)
}

/// Provides a window anchor for ASWebAuthenticationSession.
private class AuthPresentationContext: NSObject, ASWebAuthenticationPresentationContextProviding {
    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = scene.windows.first else {
            return ASPresentationAnchor()
        }
        return window
    }
}

@Observable
@MainActor
final class AuthService: AuthServiceProtocol {
    private(set) var isAuthenticated = false

    private let networkClient: NetworkClientProtocol
    private let keychainService: KeychainServiceProtocol
    private var authContinuation: CheckedContinuation<URL, Error>?
    private var authSession: ASWebAuthenticationSession?
    private let presentationContext = AuthPresentationContext()

    nonisolated static let accessTokenKey = "access_token"
    nonisolated static let refreshTokenKey = "refresh_token"

    init(networkClient: NetworkClientProtocol, keychainService: KeychainServiceProtocol) {
        self.networkClient = networkClient
        self.keychainService = keychainService

        // Check for existing valid token
        if let tokenData = try? keychainService.load(key: Self.accessTokenKey),
           let token = String(data: tokenData, encoding: .utf8),
           !JWTDecoder.isExpired(token) {
            isAuthenticated = true
        }
    }

    func loginWithGoogle() async throws {
        let authURL = try buildGoogleAuthURL()
        logger.info("Starting Google OAuth flow. URL: \(authURL.absoluteString, privacy: .private)")

        let callbackURL = try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<URL, Error>) in
            self.authContinuation = continuation

            let session = ASWebAuthenticationSession(
                url: authURL,
                callback: .customScheme("ownpulse")
            ) { [weak self] url, error in
                self?.authSession = nil
                if let error {
                    logger.error("Google OAuth error: \(error.localizedDescription, privacy: .public)")
                    continuation.resume(throwing: error)
                } else if let url {
                    logger.info("Google OAuth callback URL: \(url.absoluteString, privacy: .private)")
                    continuation.resume(returning: url)
                }
            }

            session.prefersEphemeralWebBrowserSession = false
            session.presentationContextProvider = self.presentationContext
            self.authSession = session

            let started = session.start()
            if !started {
                logger.error("Google OAuth session failed to start")
            }
        }

        try await processCallback(url: callbackURL)
    }

    func loginWithApple() async throws {
        logger.info("Starting Apple Sign-In flow")

        let credential = try await AppleAuthHelper.performAppleAuth()

        guard let idTokenData = credential.identityToken,
              let idToken = String(data: idTokenData, encoding: .utf8) else {
            logger.error("Apple Sign-In: invalid credential or missing identity token")
            throw AuthError.invalidCallback
        }

        try await processAppleCredential(idToken: idToken)
    }

    /// Testable portion of Apple Sign-In: sends the identity token to the backend,
    /// stores the returned tokens, and sets `isAuthenticated`.
    func processAppleCredential(idToken: String) async throws {
        logger.info("Apple Sign-In: received identity token, calling backend")

        let body = AppleCallbackRequest(idToken: idToken, platform: "ios")
        let response: TokenResponseWithRefresh = try await networkClient.request(
            method: "POST",
            path: Endpoints.authAppleCallback,
            body: body
        )

        try keychainService.save(key: Self.accessTokenKey, data: Data(response.accessToken.utf8))
        try keychainService.save(key: Self.refreshTokenKey, data: Data(response.refreshToken.utf8))
        isAuthenticated = true
        logger.info("Apple Sign-In: authentication successful")
    }

    func loginWithPassword(username: String, password: String) async throws {
        logger.info("Starting password login for user: \(username, privacy: .private)")

        let body = LoginRequest(username: username, password: password)
        let response: TokenResponse = try await networkClient.request(
            method: "POST",
            path: Endpoints.authLogin,
            body: body
        )

        try keychainService.save(key: Self.accessTokenKey, data: Data(response.accessToken.utf8))
        // Note: password login returns access_token only in the JSON body; refresh token is
        // set as an httpOnly cookie which iOS cannot read. Only the access token is stored.
        // Users will need to re-authenticate when the token expires (acceptable for MVP).
        isAuthenticated = true
        logger.info("Password login: authentication successful")
    }

    func handleCallback(url: URL) {
        Task {
            do {
                try await processCallback(url: url)
            } catch {
                logger.error("handleCallback failed: \(error.localizedDescription, privacy: .public)")
            }
        }
    }

    func logout() async {
        try? keychainService.delete(key: Self.accessTokenKey)
        try? keychainService.delete(key: Self.refreshTokenKey)
        isAuthenticated = false
    }

    /// Extracts tokens from the backend redirect URL.
    ///
    /// The backend completes the Google OAuth exchange and redirects to:
    ///   ownpulse://auth#token=X&refresh_token=Y
    /// or on error:
    ///   ownpulse://auth?error=...
    func processCallback(url: URL) async throws {
        guard let fragment = url.fragment else {
            // Check for error in query params: ownpulse://auth?error=...
            if let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
               let error = components.queryItems?.first(where: { $0.name == "error" })?.value {
                logger.error("Google OAuth error from backend: \(error, privacy: .public)")
                throw AuthError.callbackFailed
            }
            throw AuthError.invalidCallback
        }

        // Parse fragment as query string
        let params = fragment.split(separator: "&").reduce(into: [String: String]()) { result, pair in
            let kv = pair.split(separator: "=", maxSplits: 1)
            if kv.count == 2 { result[String(kv[0])] = String(kv[1]) }
        }

        guard let token = params["token"],
              let refreshToken = params["refresh_token"] else {
            throw AuthError.invalidCallback
        }

        try keychainService.save(key: Self.accessTokenKey, data: Data(token.utf8))
        try keychainService.save(key: Self.refreshTokenKey, data: Data(refreshToken.utf8))
        isAuthenticated = true
    }

    private func buildGoogleAuthURL() throws -> URL {
        // Go through the backend's OAuth entry point, which sets the CSRF
        // cookie and redirects to Google. The backend callback will detect
        // the mobile web view and redirect to ownpulse:// with tokens.
        guard let url = URL(string: "\(AppConfig.apiBaseURL)/api/v1/auth/google/login?platform=ios") else {
            throw AuthError.urlConstructionFailed
        }
        return url
    }
}

enum AuthError: Error, Equatable {
    case invalidCallback
    case callbackFailed
    case tokenStorageFailed
    case urlConstructionFailed
}
