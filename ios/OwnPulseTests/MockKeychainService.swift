// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
@testable import OwnPulse

final class MockKeychainService: KeychainServiceProtocol, @unchecked Sendable {
    private var store: [String: Data] = [:]
    var saveError: Error?
    private(set) var savedKeys: [String] = []

    func save(key: String, data: Data) throws {
        if let error = saveError { throw error }
        savedKeys.append(key)
        store[key] = data
    }

    func load(key: String) throws -> Data? {
        store[key]
    }

    func delete(key: String) throws {
        store.removeValue(forKey: key)
    }
}
