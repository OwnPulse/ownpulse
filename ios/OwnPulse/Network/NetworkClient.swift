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

/// Coalesces concurrent token refreshes into a single in-flight refresh.
///
/// The dashboard fires several requests in parallel; when the access token has
/// expired they each get a 401 at roughly the same time. Without coalescing
/// each one would POST the (single-use) refresh token independently, and since
/// the backend rotates the refresh token on every call, all but the first
/// rotation would invalidate the others. This coordinator guarantees that N
/// concurrent 401s trigger exactly ONE refresh — the rest await the same task.
private actor RefreshCoordinator {
    private var inFlight: Task<Void, Error>?

    /// Runs `operation` exactly once even if called concurrently. The first
    /// caller starts the task; concurrent callers await the same task. The
    /// in-flight handle is cleared once it completes so a *later* (genuinely
    /// new) refresh can start fresh.
    func refresh(_ operation: @escaping @Sendable () async throws -> Void) async throws {
        if let inFlight {
            try await inFlight.value
            return
        }
        let task = Task { try await operation() }
        inFlight = task
        defer { inFlight = nil }
        try await task.value
    }
}

final class NetworkClient: NetworkClientProtocol, @unchecked Sendable {
    private let session: URLSession
    private let keychainService: KeychainServiceProtocol
    private let decoder: JSONDecoder
    private let encoder: JSONEncoder
    private let refreshCoordinator = RefreshCoordinator()

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

        // retryCount == 1 once we've already done a 401 refresh+retry of this call.
        let retryCount = isRetry ? 1 : 0
        let start = DispatchTime.now()

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: request)
        } catch {
            // Transport failure: no HTTP status. Record status_code 0.
            recordAPICall(path: path, method: method, statusCode: 0, start: start, retryCount: retryCount)
            throw error
        }

        guard let httpResponse = response as? HTTPURLResponse else {
            recordAPICall(path: path, method: method, statusCode: 0, start: start, retryCount: retryCount)
            throw NetworkError.noData
        }

        if httpResponse.statusCode == 401 && !isRetry {
            // Record the initial 401 (retry_count 0) before attempting refresh,
            // so a refresh that throws still emits exactly one api_call event for
            // this request — the auth-failure case we most want telemetry on. On
            // success the follow-up retry records its own event with retry_count 1.
            recordAPICall(path: path, method: method, statusCode: 401, start: start, retryCount: 0)
            try await refreshToken()
            return try await performRequest(method: method, path: path, body: body, isRetry: true)
        }

        recordAPICall(
            path: path,
            method: method,
            statusCode: httpResponse.statusCode,
            start: start,
            retryCount: retryCount
        )

        guard (200...299).contains(httpResponse.statusCode) else {
            let bodyString = String(data: data, encoding: .utf8) ?? ""
            throw NetworkError.serverError(statusCode: httpResponse.statusCode, body: bodyString)
        }

        return data
    }

    /// Emit one opt-in `api_call` telemetry event. The HTTP body is never passed;
    /// only the path (normalized inside `FlowTracker` so no identifiers leave the
    /// device), method, status code, latency, and retry count are recorded.
    /// `FlowTracker.trackAPICall` is itself gated on `TelemetrySettings.isEnabled`
    /// and skips the telemetry report endpoint, so this is safe to call always.
    private func recordAPICall(
        path: String,
        method: String,
        statusCode: Int,
        start: DispatchTime,
        retryCount: Int
    ) {
        let latencyMs = Int((DispatchTime.now().uptimeNanoseconds - start.uptimeNanoseconds) / 1_000_000)
        Task {
            await FlowTracker.shared.trackAPICall(
                endpoint: path,
                method: method,
                statusCode: statusCode,
                latencyMs: latencyMs,
                retryCount: retryCount
            )
        }
    }

    /// Refreshes the session, coalescing concurrent callers into one refresh.
    ///
    /// All N concurrent 401s during the dashboard's parallel fetch share a
    /// single in-flight refresh task so the single-use refresh token is rotated
    /// exactly once. Once it completes, each original caller retries its request
    /// (with `isRetry: true`) and re-reads the freshly-saved access token from
    /// the keychain — it never triggers a second refresh.
    private func refreshToken() async throws {
        try await refreshCoordinator.refresh { [weak self] in
            guard let self else { throw NetworkError.unauthorized }
            try await self.performRefresh()
        }
    }

    /// The actual refresh network call + keychain persistence. Runs at most once
    /// per refresh round thanks to `RefreshCoordinator`.
    private func performRefresh() async throws {
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
              httpResponse.statusCode == 200,
              let tokenResponse = try? decoder.decode(TokenResponseWithRefresh.self, from: data) else {
            // Genuinely dead session: clear both tokens so the next launch
            // forces re-login instead of replaying a stale (already-rotated)
            // refresh token and looping. Mirrors AuthService.refreshAccessToken.
            try? keychainService.delete(key: AuthService.accessTokenKey)
            try? keychainService.delete(key: AuthService.refreshTokenKey)
            throw NetworkError.unauthorized
        }

        // Backend rotates the refresh token on every refresh (single-use) and
        // returns the new one in the JSON body for native clients. Persist BOTH
        // the access token and the rotated refresh token — saving only the
        // access token leaves a consumed refresh token in the keychain, which
        // breaks the next refresh and surfaces as a dashboard error on launch.
        try keychainService.save(
            key: AuthService.accessTokenKey,
            data: Data(tokenResponse.accessToken.utf8)
        )
        try keychainService.save(
            key: AuthService.refreshTokenKey,
            data: Data(tokenResponse.refreshToken.utf8)
        )
    }
}
