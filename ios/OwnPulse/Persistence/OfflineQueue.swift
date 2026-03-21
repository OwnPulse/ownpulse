// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import GRDB

struct OfflineQueueEntry: Codable, FetchableRecord, PersistableRecord, Sendable {
    static let databaseTableName = "offline_queue"

    var id: Int64?
    let payload: Data
    let createdAt: Date
    var completedAt: Date?

    enum CodingKeys: String, CodingKey {
        case id, payload
        case createdAt = "created_at"
        case completedAt = "completed_at"
    }

    enum Columns: String, ColumnExpression {
        case id, payload, createdAt = "created_at", completedAt = "completed_at"
    }
}

protocol OfflineQueueProtocol: Sendable {
    func enqueue(_ records: HealthKitBulkInsert) throws
    func dequeuePending() throws -> [(id: Int64, insert: HealthKitBulkInsert)]
    func markComplete(id: Int64) throws
}

final class OfflineQueue: OfflineQueueProtocol, Sendable {
    private let databaseManager: DatabaseManager
    private let encoder = JSONEncoder()
    private let decoder = JSONDecoder()

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
            try entry.insert(db)
        }
    }

    func dequeuePending() throws -> [(id: Int64, insert: HealthKitBulkInsert)] {
        try databaseManager.dbQueue.read { db in
            let entries = try OfflineQueueEntry
                .filter(OfflineQueueEntry.Columns.completedAt == nil)
                .order(OfflineQueueEntry.Columns.createdAt)
                .fetchAll(db)

            return entries.compactMap { entry in
                guard let id = entry.id,
                      let insert = try? decoder.decode(HealthKitBulkInsert.self, from: entry.payload) else {
                    return nil
                }
                return (id: id, insert: insert)
            }
        }
    }

    func markComplete(id: Int64) throws {
        try databaseManager.dbQueue.write { db in
            try db.execute(
                sql: "UPDATE offline_queue SET completed_at = ? WHERE id = ?",
                arguments: [Date(), id]
            )
        }
    }
}
