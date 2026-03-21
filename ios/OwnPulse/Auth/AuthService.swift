// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import AuthenticationServices
import Foundation
import Observation

@MainActor
protocol AuthServiceProtocol: Sendable {
    var isAuthenticated: Bool { get }
    func login() async throws
    func logout() async
    func handleCallback(url: URL)
}

@Observable
@MainActor
final class AuthService: AuthServiceProtocol {
    private(set) var isAuthenticated = false

    private let networkClient: NetworkClientProtocol
    private let keychainService: KeychainServiceProtocol
    private var authContinuation: CheckedContinuation<URL, Error>?

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

    func login() async throws {
        let authURL = buildGoogleAuthURL()

        let callbackURL = try await withCheckedThrowingContinuation { continuation in
            self.authContinuation = continuation

            let session = ASWebAuthenticationSession(
                url: authURL,
                callback: .customScheme("ownpulse")
            ) { url, error in
                if let error {
                    continuation.resume(throwing: error)
                } else if let url {
                    continuation.resume(returning: url)
                }
            }

            session.prefersEphemeralWebBrowserSession = false
            session.start()
        }

        try processCallback(url: callbackURL)
    }

    func handleCallback(url: URL) {
        do {
            try processCallback(url: url)
        } catch {
            // Callback handling failed — user remains unauthenticated
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
