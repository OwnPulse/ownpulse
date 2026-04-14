// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import GRDB

struct SyncAnchorRecord: Codable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "sync_anchors"

    let recordType: String
    let anchorData: Data
    let updatedAt: Date

    enum CodingKeys: String, CodingKey {
        case recordType = "record_type"
        case anchorData = "anchor_data"
        case updatedAt = "updated_at"
    }

    enum Columns: String, ColumnExpression {
        case recordType = "record_type"
        case anchorData = "anchor_data"
        case updatedAt = "updated_at"
    }
}

final class AnchorStore: Sendable {
    private let databaseManager: DatabaseManager

    init(databaseManager: DatabaseManager) {
        self.databaseManager = databaseManager
    }

    func anchor(forRecordType recordType: String) throws -> Data? {
        try databaseManager.dbQueue.read { db in
            try SyncAnchorRecord
                .filter(SyncAnchorRecord.Columns.recordType == recordType)
                .fetchOne(db)?
                .anchorData
        }
    }

    func allSyncTimestamps() throws -> [String: Date] {
        try databaseManager.dbQueue.read { db in
            let records = try SyncAnchorRecord.fetchAll(db)
            return Dictionary(uniqueKeysWithValues: records.map { ($0.recordType, $0.updatedAt) })
        }
    }

    func saveAnchor(_ data: Data, forRecordType recordType: String) throws {
        let record = SyncAnchorRecord(
            recordType: recordType,
            anchorData: data,
            updatedAt: Date()
        )
        try databaseManager.dbQueue.write { db in
            try record.save(db)
        }
    }
}
