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

    @Test("corrupt payload is marked abandoned (payload PRESERVED, not deleted) and does not block other entries from draining")
    func corruptPayloadIsAbandonedNotDeleted() throws {
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
        // ever produce, but simulates an in-flight app upgrade changing the
        // payload shape (the likeliest real-world cause), which looks
        // identical to disk corruption from the decoder's point of view.
        let corruptPayload = "not valid json".data(using: .utf8)!
        try db.dbQueue.write { db in
            try db.execute(
                sql: "INSERT INTO offline_queue (payload, created_at, completed_at, attempts, abandoned) VALUES (?, ?, NULL, 0, 0)",
                arguments: [corruptPayload, Date()]
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

        // The corrupt row must still exist — never delete queued health
        // data on a decode failure, since it could be an app-upgrade
        // artifact rather than permanent corruption — but it must be
        // marked abandoned so it stops blocking every future drain.
        let (rowCountAfter, storedPayload, abandoned) = try db.dbQueue.read { db -> (Int, Data?, Bool) in
            let count = try Int.fetchOne(db, sql: "SELECT COUNT(*) FROM offline_queue") ?? -1
            let row = try Row.fetchOne(db, sql: "SELECT payload, abandoned FROM offline_queue WHERE payload = ?", arguments: [corruptPayload])
            let payload: Data? = row?["payload"]
            let abandoned: Bool = row?["abandoned"] ?? false
            return (count, payload, abandoned)
        }
        #expect(rowCountAfter == 2, "the corrupt row must be preserved, not deleted")
        #expect(storedPayload == corruptPayload, "the payload bytes must be untouched")
        #expect(abandoned == true, "the corrupt row must be marked abandoned so it doesn't block future drains")
    }

    @Test("abandonedCount reflects abandoned rows and excludes pending/completed ones")
    func abandonedCountReflectsState() throws {
        let db = DatabaseManager(inMemory: true)
        let queue = OfflineQueue(databaseManager: db)

        #expect(try queue.abandonedCount() == 0)

        try queue.enqueue(HealthKitBulkInsert(records: []))
        try queue.enqueue(HealthKitBulkInsert(records: []))
        let pending = try queue.dequeuePending()
        #expect(pending.count == 2)

        // Abandon the first entry by hitting the retry cap.
        for _ in 0..<OfflineQueue.maxAttempts {
            try queue.recordFailedAttempt(id: pending[0].id)
        }
        #expect(try queue.abandonedCount() == 1)

        // Completing the second entry (not abandoning it) must not affect
        // the abandoned count.
        try queue.markComplete(id: pending[1].id)
        #expect(try queue.abandonedCount() == 1)
    }

    @Test("v2 migration deletes legacy completed rows left over from the old markComplete behavior")
    func migrationCleansUpLegacyCompletedRows() throws {
        // Build the database at v1 (pre-migration) directly, insert a row
        // in the old "completed_at set, never deleted" shape, THEN run the
        // full migrator — this reproduces an existing installation
        // upgrading, which `DatabaseManager.init` can't exercise directly
        // since it always migrates to latest.
        let dbQueue = try DatabaseQueue()
        var v1Only = DatabaseMigrator()
        v1Only.registerMigration("v1_create_tables") { db in
            try db.create(table: "sync_anchors") { t in
                t.primaryKey("record_type", .text)
                t.column("anchor_data", .blob).notNull()
                t.column("updated_at", .datetime).notNull()
            }
            try db.create(table: "offline_queue") { t in
                t.autoIncrementedPrimaryKey("id")
                t.column("payload", .blob).notNull()
                t.column("created_at", .datetime).notNull()
                t.column("completed_at", .datetime)
            }
        }
        try v1Only.migrate(dbQueue)

        try dbQueue.write { db in
            try db.execute(
                sql: "INSERT INTO offline_queue (payload, created_at, completed_at) VALUES (?, ?, ?)",
                arguments: [Data(), Date(), Date()]
            )
            try db.execute(
                sql: "INSERT INTO offline_queue (payload, created_at, completed_at) VALUES (?, ?, NULL)",
                arguments: [Data(), Date()]
            )
        }

        try Migrations.run(dbQueue)

        let remaining = try dbQueue.read { db in
            try Int.fetchOne(db, sql: "SELECT COUNT(*) FROM offline_queue") ?? -1
        }
        #expect(remaining == 1, "the legacy completed row must be swept by the v2 migration; the pending row must survive")
    }
}
