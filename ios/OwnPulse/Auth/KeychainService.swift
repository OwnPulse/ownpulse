// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Security

protocol KeychainServiceProtocol: Sendable {
    func save(key: String, data: Data) throws
    func load(key: String) throws -> Data?
    func delete(key: String) throws
}

enum KeychainError: Error {
    case saveFailed(OSStatus)
    case deleteFailed(OSStatus)
    case unexpectedData
}

final class KeychainService: KeychainServiceProtocol, Sendable {
    private let service = "health.ownpulse.app"

    /// Stored items (JWT access + refresh tokens) are bound to this device:
    /// available after the first unlock — so background sync/refresh keeps
    /// working while the screen is locked — but excluded from encrypted device
    /// backups and device-to-device migration. This is the strictest class that
    /// does not break background access or require a passcode to be set.
    ///
    /// Stored as `String` (not `CFString`) so the constant is `Sendable` under
    /// Swift 6 strict concurrency; `kSecAttrAccessible` accepts the bridged
    /// string value in the query dictionary.
    static let accessibility = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly as String

    func save(key: String, data: Data) throws {
        // Delete existing item first. This is also what migrates an already
        // stored item to `accessibility`: SecItemAdd does not change the
        // accessibility of an existing item, so delete-then-add ensures tokens
        // written under the old (backup-restorable) class are rewritten
        // device-bound on the next login/refresh.
        try? delete(key: key)

        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecValueData as String: data,
            kSecAttrAccessible as String: Self.accessibility,
        ]

        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw KeychainError.saveFailed(status)
        }
    }

    func load(key: String) throws -> Data? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)

        switch status {
        case errSecSuccess:
            guard let data = result as? Data else {
                throw KeychainError.unexpectedData
            }
            return data
        case errSecItemNotFound:
            return nil
        default:
            throw KeychainError.unexpectedData
        }
    }

    func delete(key: String) throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
        ]

        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw KeychainError.deleteFailed(status)
        }
    }
}
