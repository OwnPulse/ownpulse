// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("OfflineQueue")
struct OfflineQueueTests {
    @Test("enqueue and dequeue roundtrip")
    func roundtrip() throws {
        let db = DatabaseManager(inMemory: true)
        let queue = OfflineQueue(databaseManager: db)

        let record = CreateHealthRecord(
            source: "healthkit",
            recordType: "heart_rate",
            value: 72.0,
            unit: "bpm",
            startTime: Date(),
            endTime: Date(),
            metadata: nil,
            sourceId: nil
        )
        let insert = HealthKitBulkInsert(records: [record])

        try queue.enqueue(insert)

        let pending = try queue.dequeuePending()
        #expect(pending.count == 1)
        #expect(pending[0].insert.records.count == 1)
        #expect(pending[0].insert.records[0].recordType == "heart_rate")
    }

    @Test("markComplete removes from pending")
    func markComplete() throws {
        let db = DatabaseManager(inMemory: true)
        let queue = OfflineQueue(databaseManager: db)

        let insert = HealthKitBulkInsert(records: [])
        try queue.enqueue(insert)

        let pending = try queue.dequeuePending()
        #expect(pending.count == 1)

        try queue.markComplete(id: pending[0].id)

        let remaining = try queue.dequeuePending()
        #expect(remaining.isEmpty)
    }
}
