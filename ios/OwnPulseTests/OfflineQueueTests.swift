// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import GRDB
import Testing
@testable import OwnPulse

@Suite("OfflineQueue")
struct OfflineQueueTests {
    /// Forces the NEXT `recordFailedAttempt(id:)` call for `id` to be
    /// counted regardless of `OfflineQueue.minCountedAttemptInterval`, by
    /// backdating `last_counted_attempt_at` past the rate-limit window.
    /// Tests that need several counted attempts in a tight loop (no real
    /// elapsed time between calls) use this between iterations.
    private static func forceNextAttemptCounted(_ db: DatabaseManager, id: Int64) throws {
        try db.dbQueue.write { conn in
            try conn.execute(
                sql: "UPDATE offline_queue SET last_counted_attempt_at = ? WHERE id = ?",
                arguments: [Date().addingTimeInterval(-OfflineQueue.minCountedAttemptInterval - 1), id]
            )
        }
    }

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

        // Fail one short of the cap — entry should still be pending. Each
        // call is forced to count (bypassing the real-time rate limit,
        // which is covered separately below) so this test can still run
        // instantaneously.
        for _ in 0..<(OfflineQueue.maxAttempts - 1) {
            try queue.recordFailedAttempt(id: id)
            try Self.forceNextAttemptCounted(db, id: id)
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

    @Test("recordFailedAttempt does not count more than once within the minimum interval")
    func recordFailedAttemptRateLimited() throws {
        let db = DatabaseManager(inMemory: true)
        let queue = OfflineQueue(databaseManager: db)

        try queue.enqueue(HealthKitBulkInsert(records: []))
        let pending = try queue.dequeuePending()
        let id = pending[0].id

        // Two calls back-to-back (no elapsed time) — only the first counts.
        let firstAbandoned = try queue.recordFailedAttempt(id: id)
        let secondAbandoned = try queue.recordFailedAttempt(id: id)
        #expect(firstAbandoned == false)
        #expect(secondAbandoned == false)

        let attempts = try db.dbQueue.read { db in
            try Int.fetchOne(db, sql: "SELECT attempts FROM offline_queue WHERE id = ?", arguments: [id]) ?? -1
        }
        #expect(attempts == 1, "a second call within the minimum interval must not increment attempts again")
    }

    @Test("recordFailedAttempt counts again once the minimum interval has elapsed")
    func recordFailedAttemptCountsAfterInterval() throws {
        let db = DatabaseManager(inMemory: true)
        let queue = OfflineQueue(databaseManager: db)

        try queue.enqueue(HealthKitBulkInsert(records: []))
        let pending = try queue.dequeuePending()
        let id = pending[0].id

        try queue.recordFailedAttempt(id: id)
        try Self.forceNextAttemptCounted(db, id: id)
        try queue.recordFailedAttempt(id: id)

        let attempts = try db.dbQueue.read { db in
            try Int.fetchOne(db, sql: "SELECT attempts FROM offline_queue WHERE id = ?", arguments: [id]) ?? -1
        }
        #expect(attempts == 2, "a call after the minimum interval has elapsed must count")
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

    @Test("a row abandoned under one app build is retried automatically under a different build")
    func upgradeRecoversAbandonedRow() throws {
        let db = DatabaseManager(inMemory: true)

        // Insert a row with a VALID payload (so it would decode and drain
        // fine right now), but pre-marked abandoned under an OLD build —
        // simulating a row abandoned for whatever reason (undecodable
        // payload shape, or a retry-cap hit) under a previous app version.
        let validPayload = try JSONEncoder().encode(HealthKitBulkInsert(records: []))
        try db.dbQueue.write { conn in
            try conn.execute(
                sql: """
                    INSERT INTO offline_queue (payload, created_at, completed_at, attempts, abandoned, abandoned_at_build)
                    VALUES (?, ?, NULL, 10, 1, 'build-X')
                    """,
                arguments: [validPayload, Date()]
            )
        }

        // A queue instance running under a DIFFERENT build must recover it.
        let queueUnderNewBuild = OfflineQueue(databaseManager: db, currentBuild: "build-Y")
        let pending = try queueUnderNewBuild.dequeuePending()

        #expect(pending.count == 1, "a row abandoned under a different build must be retried automatically")

        let (attempts, abandoned, abandonedAtBuild) = try db.dbQueue.read { conn -> (Int, Bool, String?) in
            let row = try Row.fetchOne(conn, sql: "SELECT attempts, abandoned, abandoned_at_build FROM offline_queue")
            let attempts: Int = row?["attempts"] ?? -1
            let abandoned: Bool = row?["abandoned"] ?? true
            let abandonedAtBuild: String? = row?["abandoned_at_build"]
            return (attempts, abandoned, abandonedAtBuild)
        }
        #expect(attempts == 0, "attempts must reset on recovery")
        #expect(abandoned == false)
        #expect(abandonedAtBuild == nil)
    }

    @Test("a row abandoned under the SAME build is not recovered automatically")
    func sameBuildDoesNotRecoverAbandonedRow() throws {
        let db = DatabaseManager(inMemory: true)
        let validPayload = try JSONEncoder().encode(HealthKitBulkInsert(records: []))
        try db.dbQueue.write { conn in
            try conn.execute(
                sql: """
                    INSERT INTO offline_queue (payload, created_at, completed_at, attempts, abandoned, abandoned_at_build)
                    VALUES (?, ?, NULL, 10, 1, 'build-X')
                    """,
                arguments: [validPayload, Date()]
            )
        }

        let queueUnderSameBuild = OfflineQueue(databaseManager: db, currentBuild: "build-X")
        let pending = try queueUnderSameBuild.dequeuePending()

        #expect(pending.isEmpty, "a row abandoned under the CURRENTLY running build must stay abandoned")
    }

    @Test("retryAbandoned clears abandoned + attempt state so the entry drains again")
    func retryAbandonedClearsAndRedrains() throws {
        let db = DatabaseManager(inMemory: true)
        let queue = OfflineQueue(databaseManager: db, currentBuild: "build-X")

        try queue.enqueue(HealthKitBulkInsert(records: []))
        let pending = try queue.dequeuePending()
        let id = pending[0].id

        // Abandon it via the retry cap (same build throughout — proves
        // `retryAbandoned` is a distinct path from the automatic
        // build-based recovery in `dequeuePending`).
        for _ in 0..<OfflineQueue.maxAttempts {
            try queue.recordFailedAttempt(id: id)
            try Self.forceNextAttemptCounted(db, id: id)
        }
        #expect(try queue.abandonedCount() == 1)
        #expect(try queue.dequeuePending().isEmpty, "still on the same build — must not auto-recover")

        try queue.retryAbandoned()

        #expect(try queue.abandonedCount() == 0)
        let redrained = try queue.dequeuePending()
        #expect(redrained.count == 1, "retryAbandoned must make the entry eligible to drain again")

        let (attempts, abandoned) = try db.dbQueue.read { db -> (Int, Bool) in
            let row = try Row.fetchOne(db, sql: "SELECT attempts, abandoned FROM offline_queue WHERE id = ?", arguments: [id])
            let attempts: Int = row?["attempts"] ?? -1
            let abandoned: Bool = row?["abandoned"] ?? true
            return (attempts, abandoned)
        }
        #expect(attempts == 0)
        #expect(abandoned == false)
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
            try Self.forceNextAttemptCounted(db, id: pending[0].id)
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
