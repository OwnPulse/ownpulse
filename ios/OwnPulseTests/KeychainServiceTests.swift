// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Security
import Testing
@testable import OwnPulse

@Suite("KeychainService")
struct KeychainServiceTests {
    /// Auth tokens must be bound to the device so they are excluded from
    /// encrypted backups and device migration, while staying available after
    /// first unlock so background sync/refresh keeps working when locked.
    @Test("uses AfterFirstUnlockThisDeviceOnly accessibility")
    func usesDeviceBoundAccessibility() {
        #expect(KeychainService.accessibility == kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly)
        // Guard against the two classes that would regress this hardening:
        #expect(KeychainService.accessibility != kSecAttrAccessibleAfterFirstUnlock)
        #expect(KeychainService.accessibility != kSecAttrAccessibleWhenUnlockedThisDeviceOnly)
    }

    @Test("saves, loads, and deletes a token round-trip with device-bound accessibility")
    func roundTripSetsAccessibility() throws {
        let keychain = KeychainService()
        let key = "test_token_\(UUID().uuidString)"
        defer { try? keychain.delete(key: key) }

        let token = Data("header.payload.signature".utf8)
        try keychain.save(key: key, data: token)

        // Round-trips the stored value.
        #expect(try keychain.load(key: key) == token)

        // The stored item carries the device-bound accessibility class.
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "health.ownpulse.app",
            kSecAttrAccount as String: key,
            kSecReturnAttributes as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        #expect(status == errSecSuccess)
        let attrs = result as? [String: Any]
        let accessible = attrs?[kSecAttrAccessible as String] as? String
        #expect(accessible == (kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly as String))
    }

    @Test("save overwrites an existing item, migrating its accessibility")
    func saveUpsertsAndMigratesAccessibility() throws {
        let keychain = KeychainService()
        let key = "test_token_\(UUID().uuidString)"
        defer { try? keychain.delete(key: key) }

        // Seed an item under the old, backup-restorable accessibility class to
        // simulate a token stored before this hardening.
        let seedQuery: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "health.ownpulse.app",
            kSecAttrAccount as String: key,
            kSecValueData as String: Data("old".utf8),
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
        ]
        #expect(SecItemAdd(seedQuery as CFDictionary, nil) == errSecSuccess)

        // A subsequent save (login/refresh) must replace the value and rewrite
        // the item device-bound.
        let newToken = Data("new".utf8)
        try keychain.save(key: key, data: newToken)
        #expect(try keychain.load(key: key) == newToken)

        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "health.ownpulse.app",
            kSecAttrAccount as String: key,
            kSecReturnAttributes as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: AnyObject?
        #expect(SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess)
        let accessible = (result as? [String: Any])?[kSecAttrAccessible as String] as? String
        #expect(accessible == (kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly as String))
    }

    @Test("load returns nil for a missing key")
    func loadMissingKeyReturnsNil() throws {
        let keychain = KeychainService()
        let key = "missing_\(UUID().uuidString)"
        #expect(try keychain.load(key: key) == nil)
    }

    @Test("delete is idempotent for a missing key")
    func deleteMissingKeyDoesNotThrow() throws {
        let keychain = KeychainService()
        let key = "missing_\(UUID().uuidString)"
        try keychain.delete(key: key)
    }
}
