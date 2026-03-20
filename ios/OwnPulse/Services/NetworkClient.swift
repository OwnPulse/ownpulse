// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

/// Minimal network client protocol used by sync services.
/// The live implementation is `URLSession`-backed with JWT auth from the Keychain.
/// Tests inject a `MockNetworkClient`.
protocol NetworkClient: Sendable {
    func post<T: Decodable, U: Encodable>(_ path: String, body: U) async throws -> T
}
