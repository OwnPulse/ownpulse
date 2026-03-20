// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

protocol NetworkClientProtocol: Sendable {
    func request<T: Decodable & Sendable>(
        method: String,
        path: String,
        body: (any Encodable & Sendable)?
    ) async throws -> T

    func requestNoContent(
        method: String,
        path: String,
        body: (any Encodable & Sendable)?
    ) async throws
}

enum NetworkError: Error {
    case unauthorized
    case serverError(statusCode: Int, body: String)
    case decodingFailed(Error)
    case noData
}

final class NetworkClient: NetworkClientProtocol, @unchecked Sendable {
    private let session: URLSession
    private let keychainService: KeychainServiceProtocol
    private let decoder: JSONDecoder
    private let encoder: JSONEncoder
    private let refreshLock = NSLock()
    private var isRefreshing = false

    init(
        keychainService: KeychainServiceProtocol,
        session: URLSession = .shared
    ) {
        self.keychainService = keychainService
        self.session = session

        self.decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601

        self.encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
    }

    func request<T: Decodable & Sendable>(
        method: String,
        path: String,
        body: (any Encodable & Sendable)?
    ) async throws -> T {
        let data = try await performRequest(method: method, path: path, body: body)
        do {
            return try decoder.decode(T.self, from: data)
        } catch {
            throw NetworkError.decodingFailed(error)
        }
    }

    func requestNoContent(
        method: String,
        path: String,
        body: (any Encodable & Sendable)?
    ) async throws {
        _ = try await performRequest(method: method, path: path, body: body)
    }

    private func performRequest(
        method: String,
        path: String,
        body: (any Encodable & Sendable)?,
        isRetry: Bool = false
    ) async throws -> Data {
        var request = URLRequest(url: AppConfig.apiBaseURL.appendingPathComponent(path))
        request.httpMethod = method
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        if let tokenData = try? keychainService.load(key: AuthService.accessTokenKey),
           let token = String(data: tokenData, encoding: .utf8) {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }

        if let body {
            request.httpBody = try encoder.encode(body)
        }

        let (data, response) = try await session.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse else {
            throw NetworkError.noData
        }

        if httpResponse.statusCode == 401 && !isRetry {
            try await refreshToken()
            return try await performRequest(method: method, path: path, body: body, isRetry: true)
        }

        guard (200...299).contains(httpResponse.statusCode) else {
            let bodyString = String(data: data, encoding: .utf8) ?? ""
            throw NetworkError.serverError(statusCode: httpResponse.statusCode, body: bodyString)
        }

        return data
    }

    private func refreshToken() async throws {
        guard let refreshData = try? keychainService.load(key: AuthService.refreshTokenKey),
              let refreshToken = String(data: refreshData, encoding: .utf8) else {
            throw NetworkError.unauthorized
        }

        let refreshRequest = RefreshRequest(refreshToken: refreshToken)
        var request = URLRequest(
            url: AppConfig.apiBaseURL.appendingPathComponent(Endpoints.authRefresh)
        )
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try encoder.encode(refreshRequest)

        let (data, response) = try await session.data(for: request)

        guard let httpResponse = response as? HTTPURLResponse,
              httpResponse.statusCode == 200 else {
            throw NetworkError.unauthorized
        }

        let tokenResponse = try decoder.decode(TokenResponse.self, from: data)
        try keychainService.save(
            key: AuthService.accessTokenKey,
            data: Data(tokenResponse.accessToken.utf8)
        )
    }
}
