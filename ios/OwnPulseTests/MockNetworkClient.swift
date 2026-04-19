// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
@testable import OwnPulse

@MainActor
final class MockNetworkClient: NetworkClientProtocol, @unchecked Sendable {
    var requestHandler: ((String, String, (any Encodable & Sendable)?) throws -> Any)?
    var requestNoContentHandler: ((String, String, (any Encodable & Sendable)?) throws -> Void)?

    /// Optional async variant. When non-nil, takes precedence over
    /// `requestHandler`. Used by tests that need to stall a request mid-flight
    /// (e.g. the "events during in-flight sync" suite).
    ///
    /// Returns `any Sendable` rather than `Any` because Swift 6 refuses to
    /// send non-Sendable values across actor hops. Every response type in
    /// this codebase already conforms to `Sendable`, so callers return the
    /// same values as before — we cast to `T` at the call site the same way
    /// the sync path does.
    var asyncRequestHandler: (@Sendable (String, String, (any Encodable & Sendable)?) async throws -> any Sendable)?

    private(set) var requestCalls: [(method: String, path: String)] = []

    func request<T: Decodable & Sendable>(
        method: String,
        path: String,
        body: (any Encodable & Sendable)?
    ) async throws -> T {
        requestCalls.append((method: method, path: path))

        let result: Any
        if let asyncHandler = asyncRequestHandler {
            result = try await asyncHandler(method, path, body)
        } else if let handler = requestHandler {
            result = try handler(method, path, body)
        } else {
            fatalError("MockNetworkClient.requestHandler not set")
        }

        guard let typed = result as? T else {
            fatalError("MockNetworkClient handler returned wrong type")
        }
        return typed
    }

    func requestNoContent(
        method: String,
        path: String,
        body: (any Encodable & Sendable)?
    ) async throws {
        requestCalls.append((method: method, path: path))
        try requestNoContentHandler?(method, path, body)
    }
}
