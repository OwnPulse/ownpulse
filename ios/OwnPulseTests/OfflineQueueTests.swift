// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import GRDB
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

    @Test("markComplete deletes the row entirely, not just marks it")
    func markCompleteDeletesRow() throws {
        let db = DatabaseManager(inMemory: true)
        let queue = OfflineQueue(databaseManager: db)

        try queue.enqueue(HealthKitBulkInsert(records: []))
        let pending = try queue.dequeuePending()
        #expect(pending.count == 1)

        try queue.markComplete(id: pending[0].id)

        let rowCount = try db.dbQueue.read { db in
            try Int.fetchOne(db, sql: "SELECT COUNT(*) FROM offline_queue") ?? -1
        }
        #expect(rowCount == 0, "completed rows must be deleted, not accumulate forever")
    }

    @Test("recordFailedAttempt increments attempts and abandons at the cap")
    func recordFailedAttemptAbandonsAtCap() throws {
        let db = DatabaseManager(inMemory: true)
        let queue = OfflineQueue(databaseManager: db)

        try queue.enqueue(HealthKitBulkInsert(records: []))
        let pending = try queue.dequeuePending()
        #expect(pending.count == 1)
        let id = pending[0].id

        // Fail one short of the cap — entry should still be pending.
        for _ in 0..<(OfflineQueue.maxAttempts - 1) {
            try queue.recordFailedAttempt(id: id)
        }
        #expect(try queue.dequeuePending().count == 1, "entry should still be pending before the cap is reached")

        // One more failure hits the cap and abandons the entry.
        try queue.recordFailedAttempt(id: id)

        let afterCap = try queue.dequeuePending()
        #expect(afterCap.isEmpty, "entry must be excluded from pending once abandoned")

        let (attempts, abandoned) = try db.dbQueue.read { db -> (Int, Bool) in
            let row = try Row.fetchOne(db, sql: "SELECT attempts, abandoned FROM offline_queue WHERE id = ?", arguments: [id])
            let attempts: Int = row?["attempts"] ?? -1
            let abandoned: Bool = row?["abandoned"] ?? false
            return (attempts, abandoned)
        }
        #expect(attempts == OfflineQueue.maxAttempts)
        #expect(abandoned == true)
    }

    @Test("corrupt payload is deleted on dequeue and does not block other entries from draining")
    func corruptPayloadIsRemoved() throws {
        let db = DatabaseManager(inMemory: true)
        let queue = OfflineQueue(databaseManager: db)

        // A valid entry that should survive and drain normally.
        let validRecord = CreateHealthRecord(
            source: "healthkit",
            recordType: "heart_rate",
            value: 72.0,
            unit: "bpm",
            startTime: Date(),
            endTime: Date(),
            metadata: nil,
            sourceId: "ok-1"
        )
        try queue.enqueue(HealthKitBulkInsert(records: [validRecord]))

        // A corrupt row inserted directly — not something `enqueue` could
        // ever produce, but simulates disk corruption / a future payload
        // schema change that makes an old row undecodable.
        try db.dbQueue.write { db in
            try db.execute(
                sql: "INSERT INTO offline_queue (payload, created_at, completed_at, attempts, abandoned) VALUES (?, ?, NULL, 0, 0)",
                arguments: ["not valid json".data(using: .utf8)!, Date()]
            )
        }

        let rowCountBefore = try db.dbQueue.read { db in
            try Int.fetchOne(db, sql: "SELECT COUNT(*) FROM offline_queue") ?? -1
        }
        #expect(rowCountBefore == 2)

        let pending = try queue.dequeuePending()

        // Only the valid entry drains.
        #expect(pending.count == 1)
        #expect(pending[0].insert.records.first?.sourceId == "ok-1")

        // The corrupt row was deleted, not left forever blocking future drains.
        let rowCountAfter = try db.dbQueue.read { db in
            try Int.fetchOne(db, sql: "SELECT COUNT(*) FROM offline_queue") ?? -1
        }
        #expect(rowCountAfter == 1)
    }
}
