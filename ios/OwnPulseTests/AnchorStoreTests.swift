// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("AnchorStore")
struct AnchorStoreTests {
    @Test("save and load anchor roundtrip")
    func roundtrip() throws {
        let db = DatabaseManager(inMemory: true)
        let store = AnchorStore(databaseManager: db)

        let anchorData = Data("test-anchor".utf8)
        try store.saveAnchor(anchorData, forRecordType: "heart_rate")

        let loaded = try store.anchor(forRecordType: "heart_rate")
        #expect(loaded == anchorData)
    }

    @Test("returns nil for unknown type")
    func unknownType() throws {
        let db = DatabaseManager(inMemory: true)
        let store = AnchorStore(databaseManager: db)

        let loaded = try store.anchor(forRecordType: "nonexistent")
        #expect(loaded == nil)
    }

    @Test("overwrites existing anchor")
    func overwrite() throws {
        let db = DatabaseManager(inMemory: true)
        let store = AnchorStore(databaseManager: db)

        try store.saveAnchor(Data("first".utf8), forRecordType: "steps")
        try store.saveAnchor(Data("second".utf8), forRecordType: "steps")

        let loaded = try store.anchor(forRecordType: "steps")
        #expect(loaded == Data("second".utf8))
    }
}
