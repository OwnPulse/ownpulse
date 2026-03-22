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

    /// Ephemeral PKCE code verifier — held in memory only for the duration of a login attempt.
    private var codeVerifier: String?

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

    /// Extracts the authorization code from Google's redirect, then calls the backend
    /// with the stored code_verifier so the backend can complete the PKCE exchange.
    private func processCallback(url: URL) async throws {
        guard let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
              let code = components.queryItems?.first(where: { $0.name == "code" })?.value
        else {
            throw AuthError.invalidCallback
        }

        guard let verifier = codeVerifier else {
            throw AuthError.missingCodeVerifier
        }
        // Clear the verifier immediately — single use.
        codeVerifier = nil

        var backendComponents = URLComponents(
            url: AppConfig.apiBaseURL.appendingPathComponent(Endpoints.authGoogleCallback),
            resolvingAgainstBaseURL: false
        )!
        backendComponents.queryItems = [
            URLQueryItem(name: "code", value: code),
            URLQueryItem(name: "code_verifier", value: verifier),
        ]

        guard let callbackURL = backendComponents.url else {
            throw AuthError.invalidCallback
        }

        var request = URLRequest(url: callbackURL)
        request.httpMethod = "GET"

        let (data, response) = try await URLSession.shared.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse,
              (200...299).contains(httpResponse.statusCode) else {
            throw AuthError.callbackFailed
        }

        let decoder = JSONDecoder()
        let authResponse = try decoder.decode(AuthCallbackResponse.self, from: data)

        try keychainService.save(
            key: Self.accessTokenKey,
            data: Data(authResponse.token.utf8)
        )
        try keychainService.save(
            key: Self.refreshTokenKey,
            data: Data(authResponse.refreshToken.utf8)
        )

        isAuthenticated = true
    }

    private func buildGoogleAuthURL() throws -> URL {
        let verifier = PKCEHelper.generateCodeVerifier()
        codeVerifier = verifier
        let challenge = PKCEHelper.codeChallenge(from: verifier)

        guard var components = URLComponents(string: "https://accounts.google.com/o/oauth2/v2/auth") else {
            throw AuthError.urlConstructionFailed
        }
        components.queryItems = [
            URLQueryItem(name: "client_id", value: AppConfig.googleClientID),
            URLQueryItem(name: "redirect_uri", value: AppConfig.googleRedirectURI),
            URLQueryItem(name: "response_type", value: "code"),
            URLQueryItem(name: "scope", value: "openid email"),
            URLQueryItem(name: "code_challenge", value: challenge),
            URLQueryItem(name: "code_challenge_method", value: "S256"),
        ]
        guard let url = components.url else {
            throw AuthError.urlConstructionFailed
        }
        return url
    }
}

enum AuthError: Error {
    case invalidCallback
    case missingCodeVerifier
    case callbackFailed
    case tokenStorageFailed
    case urlConstructionFailed
}
