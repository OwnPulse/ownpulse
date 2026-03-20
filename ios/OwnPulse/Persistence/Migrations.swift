// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import GRDB

enum Migrations {
    static func run(_ db: DatabaseQueue) throws {
        var migrator = DatabaseMigrator()

        migrator.registerMigration("v1_create_tables") { db in
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

        try migrator.migrate(db)
    }
}
