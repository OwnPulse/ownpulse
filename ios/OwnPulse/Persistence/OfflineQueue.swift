// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import GRDB
import os

private let offlineQueueLogger = Logger(subsystem: "health.ownpulse.app", category: "offline-queue")

struct OfflineQueueEntry: Codable, FetchableRecord, PersistableRecord, Sendable {
    static let databaseTableName = "offline_queue"

    var id: Int64?
    let payload: Data
    let createdAt: Date
    var completedAt: Date?
    var attempts: Int = 0
    var abandoned: Bool = false
    var abandonedAtBuild: String?
    var lastCountedAttemptAt: Date?

    enum CodingKeys: String, CodingKey {
        case id, payload, attempts, abandoned
        case createdAt = "created_at"
        case completedAt = "completed_at"
        case abandonedAtBuild = "abandoned_at_build"
        case lastCountedAttemptAt = "last_counted_attempt_at"
    }

    enum Columns: String, ColumnExpression {
        case id, payload, createdAt = "created_at", completedAt = "completed_at"
        case attempts, abandoned
        case abandonedAtBuild = "abandoned_at_build"
        case lastCountedAttemptAt = "last_counted_attempt_at"
    }
}

protocol OfflineQueueProtocol: Sendable {
    func enqueue(_ records: HealthKitBulkInsert) throws
    func dequeuePending() throws -> [(id: Int64, insert: HealthKitBulkInsert)]
    func markComplete(id: Int64) throws
    /// Records a failed drain attempt for `id`. Rate-limited to at most one
    /// COUNTED attempt per `OfflineQueue.minCountedAttemptInterval` — a
    /// burst of failures within that window (e.g. the observer debounce
    /// firing syncs every few seconds) doesn't burn the retry budget any
    /// faster than the interval allows. After `OfflineQueue.maxAttempts`
    /// counted failures the entry is marked abandoned so it no longer
    /// blocks every future sync's offline-queue drain. Callers must only
    /// invoke this for deterministic rejections (see `SyncEngine`'s error
    /// classification) — never for transport/connectivity failures, or a
    /// network outage would abandon the whole queue.
    /// - Returns: `true` if this call caused the entry to become abandoned.
    @discardableResult
    func recordFailedAttempt(id: Int64) throws -> Bool
    /// Count of currently-abandoned entries, surfaced to the UI so drops are
    /// never silent.
    func abandonedCount() throws -> Int
    /// Clears abandoned + attempt state for every currently-abandoned entry
    /// so they're eligible to drain again. The user-initiated recovery
    /// affordance (a "Retry" action) — abandonment must never be a
    /// permanent dead end.
    func retryAbandoned() throws
}

final class OfflineQueue: OfflineQueueProtocol, Sendable {
    private let databaseManager: DatabaseManager
    private let encoder = JSONEncoder()
    private let decoder = JSONDecoder()
    private let currentBuild: String

    /// Cap on counted retry attempts before an offline-queue entry is
    /// abandoned. Chosen to give transient failures (network blip, brief
    /// backend outage) plenty of chances to resolve — combined with
    /// `minCountedAttemptInterval`, the cap can only be reached after at
    /// least `(maxAttempts - 1) * minCountedAttemptInterval` of real
    /// elapsed time — while still eventually giving up on entries the
    /// backend will never accept (e.g. a hard 4xx that will never turn
    /// into a 2xx).
    static let maxAttempts = 10

    /// Minimum elapsed time between COUNTED attempts for the same entry.
    /// A sync pass only ever calls `recordFailedAttempt` once per entry
    /// (each pending entry is drained at most once per `drainOfflineQueue`
    /// call), but the observer debounce can trigger a new sync pass every
    /// few seconds — without this floor a transient backend/proxy problem
    /// lasting only tens of seconds could still burn through the whole
    /// `maxAttempts` budget and abandon the entry.
    static let minCountedAttemptInterval: TimeInterval = 600

    init(databaseManager: DatabaseManager, currentBuild: String = AppConfig.buildNumber) {
        self.databaseManager = databaseManager
        self.currentBuild = currentBuild
    }

    func enqueue(_ records: HealthKitBulkInsert) throws {
        let payload = try encoder.encode(records)
        let entry = OfflineQueueEntry(
            payload: payload,
            createdAt: Date(),
            completedAt: nil
        )
        try databaseManager.dbQueue.write { db in
            let mutableEntry = entry
            try mutableEntry.insert(db)
        }
    }

    func dequeuePending() throws -> [(id: Int64, insert: HealthKitBulkInsert)] {
        try databaseManager.dbQueue.write { db in
            // Abandonment must always be recoverable. A row abandoned
            // under a DIFFERENT app build than the one running now gets a
            // fresh chance automatically — whatever abandoned it (an
            // undecodable payload shape that a newer build may parse
            // fine, or a retry-cap hit under different app logic) isn't
            // guaranteed to still apply. `abandoned_at_build IS NULL`
            // covers rows abandoned before this column existed.
            try Self.recoverStaleAbandonments(db, currentBuild: currentBuild)

            let entries = try OfflineQueueEntry
                .filter(OfflineQueueEntry.Columns.completedAt == nil)
                .filter(OfflineQueueEntry.Columns.abandoned == false)
                .order(OfflineQueueEntry.Columns.createdAt)
                .fetchAll(db)

            var result: [(id: Int64, insert: HealthKitBulkInsert)] = []
            result.reserveCapacity(entries.count)
            for entry in entries {
                guard let id = entry.id else { continue }
                do {
                    let insert = try decoder.decode(HealthKitBulkInsert.self, from: entry.payload)
                    result.append((id: id, insert: insert))
                } catch {
                    // A payload undecodable by the CURRENT app version isn't
                    // necessarily permanently corrupt — an in-flight app
                    // upgrade changing the payload shape looks identical.
                    // Never delete queued health data on a decode failure;
                    // mark it abandoned (payload preserved, recoverable on
                    // the next build change via the sweep above) so it
                    // stops blocking every future drain, and log only the
                    // error's TYPE + row id — `DecodingError.dataCorrupted`
                    // embeds the underlying `JSONSerialization` error, whose
                    // `debugDescription` can quote the offending payload
                    // (health data), so its string form must never be logged.
                    try db.execute(
                        sql: "UPDATE offline_queue SET abandoned = 1, abandoned_at_build = ? WHERE id = ?",
                        arguments: [currentBuild, id]
                    )
                    offlineQueueLogger.error("offline_queue: undecodable payload marked abandoned id=\(id, privacy: .public) errorType=\(String(describing: type(of: error)), privacy: .public)")
                }
            }
            return result
        }
    }

    func markComplete(id: Int64) throws {
        try databaseManager.dbQueue.write { db in
            try db.execute(
                sql: "DELETE FROM offline_queue WHERE id = ?",
                arguments: [id]
            )
        }
    }

    @discardableResult
    func recordFailedAttempt(id: Int64) throws -> Bool {
        try databaseManager.dbQueue.write { db in
            let now = Date()
            let lastCounted = try Date.fetchOne(
                db,
                sql: "SELECT last_counted_attempt_at FROM offline_queue WHERE id = ?",
                arguments: [id]
            )
            if let lastCounted, now.timeIntervalSince(lastCounted) < Self.minCountedAttemptInterval {
                // Too soon since the last COUNTED attempt for this entry —
                // don't burn the retry budget faster than the interval
                // allows.
                return false
            }

            try db.execute(
                sql: "UPDATE offline_queue SET attempts = attempts + 1, last_counted_attempt_at = ? WHERE id = ?",
                arguments: [now, id]
            )
            let attempts = try Int.fetchOne(
                db,
                sql: "SELECT attempts FROM offline_queue WHERE id = ?",
                arguments: [id]
            )
            guard let attempts, attempts >= Self.maxAttempts else { return false }
            try db.execute(
                sql: "UPDATE offline_queue SET abandoned = 1, abandoned_at_build = ? WHERE id = ?",
                arguments: [currentBuild, id]
            )
            offlineQueueLogger.warning("offline_queue: abandoning entry id=\(id, privacy: .public) after \(attempts, privacy: .public) counted failed attempts")
            return true
        }
    }

    func abandonedCount() throws -> Int {
        try databaseManager.dbQueue.read { db in
            try Int.fetchOne(db, sql: "SELECT COUNT(*) FROM offline_queue WHERE abandoned = 1") ?? 0
        }
    }

    func retryAbandoned() throws {
        try databaseManager.dbQueue.write { db in
            try db.execute(
                sql: """
                    UPDATE offline_queue
                    SET abandoned = 0, attempts = 0, last_counted_attempt_at = NULL, abandoned_at_build = NULL
                    WHERE abandoned = 1
                    """
            )
        }
    }

    /// Resets abandoned rows stamped under a build other than `currentBuild`
    /// (or stamped before this column existed) so they drain again. Shared
    /// by `dequeuePending` (automatic, on every drain) — `retryAbandoned`
    /// is the separate user-initiated path that clears ALL abandoned rows
    /// regardless of build.
    private static func recoverStaleAbandonments(_ db: Database, currentBuild: String) throws {
        try db.execute(
            sql: """
                UPDATE offline_queue
                SET abandoned = 0, attempts = 0, last_counted_attempt_at = NULL, abandoned_at_build = NULL
                WHERE abandoned = 1 AND (abandoned_at_build IS NULL OR abandoned_at_build != ?)
                """,
            arguments: [currentBuild]
        )
    }
}
