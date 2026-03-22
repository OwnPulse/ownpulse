// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
@testable import OwnPulse

final class MockNetworkClient: NetworkClientProtocol, @unchecked Sendable {
    var requestHandler: ((String, String, (any Encodable & Sendable)?) throws -> Any)?
    var requestNoContentHandler: ((String, String, (any Encodable & Sendable)?) throws -> Void)?

    private(set) var requestCalls: [(method: String, path: String)] = []

    func request<T: Decodable & Sendable>(
        method: String,
        path: String,
        body: (any Encodable & Sendable)?
    ) async throws -> T {
        requestCalls.append((method: method, path: path))
        guard let handler = requestHandler else {
            fatalError("MockNetworkClient.requestHandler not set")
        }
        let result = try handler(method, path, body)
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
