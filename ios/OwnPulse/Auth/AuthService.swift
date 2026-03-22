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
        let authURL = buildGoogleAuthURL()
        logger.info("Starting Google OAuth flow. URL: \(authURL.absoluteString, privacy: .public)")

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
                    logger.info("Google OAuth callback URL: \(url.absoluteString, privacy: .public)")
                    continuation.resume(returning: url)
                }
            }

            session.prefersEphemeralWebBrowserSession = false
            session.presentationContextProvider = self.presentationContext
            self.authSession = session

            let started = session.start()
            logger.info("ASWebAuthenticationSession.start() returned: \(started)")
            if !started {
                logger.error("Google OAuth session failed to start")
            }
        }

        try processCallback(url: callbackURL)
    }

    func loginWithApple() async throws {
        logger.info("Starting Apple Sign-In flow")

        let credential = try await AppleAuthHelper.performAppleAuth()

        guard let idTokenData = credential.identityToken,
              let idToken = String(data: idTokenData, encoding: .utf8) else {
            logger.error("Apple Sign-In: invalid credential or missing identity token")
            throw AuthError.invalidCallback
        }

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
        do {
            try processCallback(url: url)
        } catch {
            logger.error("handleCallback failed: \(error.localizedDescription, privacy: .public)")
        }
    }

    func logout() async {
        try? keychainService.delete(key: Self.accessTokenKey)
        try? keychainService.delete(key: Self.refreshTokenKey)
        isAuthenticated = false
    }

    private func processCallback(url: URL) throws {
        guard let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
              let token = components.queryItems?.first(where: { $0.name == "token" })?.value,
              let refreshToken = components.queryItems?.first(where: { $0.name == "refresh_token" })?.value
        else {
            throw AuthError.invalidCallback
        }

        try keychainService.save(
            key: Self.accessTokenKey,
            data: Data(token.utf8)
        )
        try keychainService.save(
            key: Self.refreshTokenKey,
            data: Data(refreshToken.utf8)
        )

        isAuthenticated = true
    }

    private func buildGoogleAuthURL() -> URL {
        var components = URLComponents(string: "https://accounts.google.com/o/oauth2/v2/auth")!
        components.queryItems = [
            URLQueryItem(name: "client_id", value: AppConfig.googleClientID),
            URLQueryItem(name: "redirect_uri", value: AppConfig.googleRedirectURI),
            URLQueryItem(name: "response_type", value: "code"),
            URLQueryItem(name: "scope", value: "openid email"),
            URLQueryItem(name: "state", value: "ios"),
        ]
        return components.url!
    }
}

enum AuthError: Error {
    case invalidCallback
    case tokenStorageFailed
}
