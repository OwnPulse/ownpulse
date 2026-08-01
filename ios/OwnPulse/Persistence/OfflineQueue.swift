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

    enum CodingKeys: String, CodingKey {
        case id, payload, attempts, abandoned
        case createdAt = "created_at"
        case completedAt = "completed_at"
    }

    enum Columns: String, ColumnExpression {
        case id, payload, createdAt = "created_at", completedAt = "completed_at"
        case attempts, abandoned
    }
}

protocol OfflineQueueProtocol: Sendable {
    func enqueue(_ records: HealthKitBulkInsert) throws
    func dequeuePending() throws -> [(id: Int64, insert: HealthKitBulkInsert)]
    func markComplete(id: Int64) throws
    /// Records a failed drain attempt for `id`. After `OfflineQueue.maxAttempts`
    /// failed attempts the entry is marked abandoned so it no longer blocks
    /// every future sync's offline-queue drain. Callers must only invoke this
    /// for deterministic rejections (see `SyncEngine`'s error classification)
    /// — never for transport/connectivity failures, or a network outage
    /// would abandon the whole queue.
    /// - Returns: `true` if this call caused the entry to become abandoned.
    @discardableResult
    func recordFailedAttempt(id: Int64) throws -> Bool
    /// Count of currently-abandoned entries, surfaced to the UI so drops are
    /// never silent.
    func abandonedCount() throws -> Int
}

final class OfflineQueue: OfflineQueueProtocol, Sendable {
    private let databaseManager: DatabaseManager
    private let encoder = JSONEncoder()
    private let decoder = JSONDecoder()

    /// Cap on retry attempts before an offline-queue entry is abandoned.
    /// Chosen to give transient failures (network blip, brief backend
    /// outage) plenty of chances to resolve while still eventually giving
    /// up on entries the backend will never accept (e.g. a hard 4xx that
    /// will never turn into a 2xx).
    static let maxAttempts = 10

    init(databaseManager: DatabaseManager) {
        self.databaseManager = databaseManager
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
                    // mark it abandoned (payload preserved) so it stops
                    // blocking every future drain, and log only the error's
                    // TYPE + row id — `DecodingError.dataCorrupted` embeds
                    // the underlying `JSONSerialization` error, whose
                    // `debugDescription` can quote the offending payload
                    // (health data), so its string form must never be logged.
                    try db.execute(sql: "UPDATE offline_queue SET abandoned = 1 WHERE id = ?", arguments: [id])
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
            try db.execute(
                sql: "UPDATE offline_queue SET attempts = attempts + 1 WHERE id = ?",
                arguments: [id]
            )
            let attempts = try Int.fetchOne(
                db,
                sql: "SELECT attempts FROM offline_queue WHERE id = ?",
                arguments: [id]
            )
            guard let attempts, attempts >= Self.maxAttempts else { return false }
            try db.execute(
                sql: "UPDATE offline_queue SET abandoned = 1 WHERE id = ?",
                arguments: [id]
            )
            offlineQueueLogger.warning("offline_queue: abandoning entry id=\(id, privacy: .public) after \(attempts, privacy: .public) failed attempts")
            return true
        }
    }

    func abandonedCount() throws -> Int {
        try databaseManager.dbQueue.read { db in
            try Int.fetchOne(db, sql: "SELECT COUNT(*) FROM offline_queue WHERE abandoned = 1") ?? 0
        }
    }
}
