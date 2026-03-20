// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

/// Test double for `NetworkClient`.
///
/// Typical usage:
/// ```swift
/// let client = MockNetworkClient()
/// client.stubbedResponse = someEncodedSleepRecord
/// // ... call service under test ...
/// #expect(client.postedBodies.count == 1)
/// ```
final class MockNetworkClient: NetworkClient, @unchecked Sendable {

    // MARK: - Stub configuration

    /// JSON `Data` to decode and return from `post(_:body:)`.
    /// Set this to a valid encoding of the expected return type.
    var stubbedResponseData: Data = Data()

    /// When non-nil, `post` throws this error instead of decoding `stubbedResponseData`.
    var stubbedError: (any Error)?

    // MARK: - Call tracking

    /// Raw encoded bodies from every `post` call, in order.
    private(set) var postedBodies: [Data] = []

    /// Paths from every `post` call, in order.
    private(set) var postedPaths: [String] = []

    private let encoder = JSONEncoder()
    private let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.dateDecodingStrategy = .iso8601
        return d
    }()

    // MARK: - NetworkClient

    func post<T: Decodable, U: Encodable>(_ path: String, body: U) async throws -> T {
        postedPaths.append(path)

        let encoded = try encoder.encode(body)
        postedBodies.append(encoded)

        if let error = stubbedError {
            throw error
        }

        return try decoder.decode(T.self, from: stubbedResponseData)
    }
}
