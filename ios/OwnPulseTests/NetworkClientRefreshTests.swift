// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

/// A `URLProtocol` stub that drives the `NetworkClient` 401 -> refresh -> retry
/// path end to end.
///
/// It serves three kinds of responses, keyed off the request path:
///   - the refresh endpoint (`Endpoints.authRefresh`): a 200 with a rotated
///     refresh token, or a configurable failure status
///   - any protected endpoint: a 401 on the first hit, then 200 once a refresh
///     has happened (so the retry succeeds)
///
/// It counts how many times the refresh endpoint was hit so concurrency tests
/// can assert exactly one refresh across N parallel callers.
final class RefreshStubProtocol: URLProtocol, @unchecked Sendable {
    /// HTTP status the refresh endpoint returns. 200 = success.
    nonisolated(unsafe) static var refreshStatus = 200
    /// The rotated refresh token the refresh endpoint returns in its body.
    nonisolated(unsafe) static var rotatedRefreshToken = "rotated-refresh"
    /// The access token the refresh endpoint returns in its body.
    nonisolated(unsafe) static var rotatedAccessToken = "rotated-access"
    /// Artificial delay (seconds) before the refresh endpoint responds, used to
    /// widen the race window in concurrency tests.
    nonisolated(unsafe) static var refreshDelay: TimeInterval = 0

    private static let lock = NSLock()
    nonisolated(unsafe) private static var _refreshCount = 0
    nonisolated(unsafe) private static var _refreshed = false

    static var refreshCount: Int {
        lock.withLock { _refreshCount }
    }

    static func reset() {
        lock.withLock {
            _refreshCount = 0
            _refreshed = false
        }
        refreshStatus = 200
        rotatedRefreshToken = "rotated-refresh"
        rotatedAccessToken = "rotated-access"
        refreshDelay = 0
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let path = request.url?.path ?? ""

        if path == Endpoints.authRefresh {
            handleRefresh()
        } else {
            handleProtected()
        }
    }

    private func handleRefresh() {
        Self.lock.withLock { Self._refreshCount += 1 }

        let respond = { [self] in
            if Self.refreshStatus == 200 {
                Self.lock.withLock { Self._refreshed = true }
                let body = Data("""
                {
                  "access_token": "\(Self.rotatedAccessToken)",
                  "refresh_token": "\(Self.rotatedRefreshToken)",
                  "token_type": "Bearer",
                  "expires_in": 3600
                }
                """.utf8)
                finish(status: 200, body: body)
            } else {
                finish(status: Self.refreshStatus, body: Data("{}".utf8))
            }
        }

        if Self.refreshDelay > 0 {
            DispatchQueue.global().asyncAfter(deadline: .now() + Self.refreshDelay) { respond() }
        } else {
            respond()
        }
    }

    private func handleProtected() {
        let refreshed = Self.lock.withLock { Self._refreshed }
        if refreshed {
            finish(status: 200, body: Data("""
            {"ok": true}
            """.utf8))
        } else {
            finish(status: 401, body: Data("{}".utf8))
        }
    }

    private func finish(status: Int, body: Data) {
        let response = HTTPURLResponse(
            url: request.url!,
            statusCode: status,
            httpVersion: nil,
            headerFields: ["Content-Type": "application/json"]
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: body)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}

    static func session() -> URLSession {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [RefreshStubProtocol.self]
        return URLSession(configuration: config)
    }
}

private struct OKResponse: Decodable, Sendable {
    let ok: Bool
}

@Suite("NetworkClient refresh-token rotation", .serialized)
struct NetworkClientRefreshTests {
    private func makeClient(keychain: MockKeychainService) -> NetworkClient {
        NetworkClient(keychainService: keychain, session: RefreshStubProtocol.session())
    }

    private func seedTokens(_ keychain: MockKeychainService) {
        try! keychain.save(key: AuthService.accessTokenKey, data: Data("stale-access".utf8))
        try! keychain.save(key: AuthService.refreshTokenKey, data: Data("old-refresh".utf8))
    }

    @Test("401 then successful refresh persists BOTH rotated tokens")
    func refreshPersistsRotatedTokens() async throws {
        RefreshStubProtocol.reset()
        RefreshStubProtocol.rotatedAccessToken = "fresh-access"
        RefreshStubProtocol.rotatedRefreshToken = "fresh-refresh"

        let keychain = MockKeychainService()
        seedTokens(keychain)
        let client = makeClient(keychain: keychain)

        let result: OKResponse = try await client.request(
            method: "GET", path: "/api/v1/dashboard/summary", body: nil
        )
        #expect(result.ok)

        let access = String(data: try keychain.load(key: AuthService.accessTokenKey)!, encoding: .utf8)
        let refresh = String(data: try keychain.load(key: AuthService.refreshTokenKey)!, encoding: .utf8)
        // The rotated refresh token must have replaced the consumed old one.
        #expect(access == "fresh-access")
        #expect(refresh == "fresh-refresh")
        #expect(refresh != "old-refresh")
        #expect(RefreshStubProtocol.refreshCount == 1)
    }

    @Test("failed refresh clears both tokens and throws .unauthorized")
    func failedRefreshClearsTokens() async throws {
        RefreshStubProtocol.reset()
        RefreshStubProtocol.refreshStatus = 401

        let keychain = MockKeychainService()
        seedTokens(keychain)
        let client = makeClient(keychain: keychain)

        await #expect(throws: NetworkError.self) {
            let _: OKResponse = try await client.request(
                method: "GET", path: "/api/v1/dashboard/summary", body: nil
            )
        }

        #expect((try keychain.load(key: AuthService.accessTokenKey)) == nil)
        #expect((try keychain.load(key: AuthService.refreshTokenKey)) == nil)
    }

    @Test("missing refresh token throws .unauthorized without hitting the network")
    func missingRefreshTokenUnauthorized() async throws {
        RefreshStubProtocol.reset()

        let keychain = MockKeychainService()
        // Only an access token, no refresh token.
        try keychain.save(key: AuthService.accessTokenKey, data: Data("stale-access".utf8))
        let client = makeClient(keychain: keychain)

        await #expect(throws: NetworkError.self) {
            let _: OKResponse = try await client.request(
                method: "GET", path: "/api/v1/dashboard/summary", body: nil
            )
        }
        #expect(RefreshStubProtocol.refreshCount == 0)
    }

    @Test("concurrent 401s coalesce into exactly one refresh; all requests succeed")
    func concurrentRefreshesCoalesce() async throws {
        RefreshStubProtocol.reset()
        // Hold the refresh open briefly so all parallel callers pile up on the
        // in-flight task before it resolves.
        RefreshStubProtocol.refreshDelay = 0.15

        let keychain = MockKeychainService()
        seedTokens(keychain)
        let client = makeClient(keychain: keychain)

        try await withThrowingTaskGroup(of: Bool.self) { group in
            for path in ["/summary", "/sparklines", "/insights", "/hero", "/today", "/weekly"] {
                group.addTask {
                    let r: OKResponse = try await client.request(
                        method: "GET", path: "/api/v1/dashboard\(path)", body: nil
                    )
                    return r.ok
                }
            }
            var successes = 0
            for try await ok in group where ok { successes += 1 }
            #expect(successes == 6)
        }

        // The crux: N parallel 401s must rotate the single-use refresh token
        // exactly once, not once per request.
        #expect(RefreshStubProtocol.refreshCount == 1)
    }

    @Test("regression: dashboard-style parallel fetch survives an expired access token")
    func dashboardParallelFetchRegression() async throws {
        RefreshStubProtocol.reset()
        RefreshStubProtocol.refreshDelay = 0.1
        RefreshStubProtocol.rotatedRefreshToken = "rotated-once"

        let keychain = MockKeychainService()
        seedTokens(keychain)
        let client = makeClient(keychain: keychain)

        // Mirror DashboardViewModel.loadDashboard's parallel request fan-out.
        async let summary: OKResponse = client.request(method: "GET", path: "/api/v1/dashboard/summary", body: nil)
        async let sparklines: OKResponse = client.request(method: "GET", path: "/api/v1/dashboard/sparklines", body: nil)
        async let insights: OKResponse = client.request(method: "GET", path: "/api/v1/dashboard/insights", body: nil)
        async let hero: OKResponse = client.request(method: "GET", path: "/api/v1/dashboard/hero", body: nil)

        let results = try await [summary, sparklines, insights, hero]
        #expect(results.allSatisfy { $0.ok })
        #expect(RefreshStubProtocol.refreshCount == 1)

        let refresh = String(data: try keychain.load(key: AuthService.refreshTokenKey)!, encoding: .utf8)
        #expect(refresh == "rotated-once")
    }
}
