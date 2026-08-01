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

        // Adds queue-hygiene columns: `attempts` tracks failed drain
        // attempts so a permanently-unreachable entry can be abandoned
        // instead of blocking every future sync retry; `abandoned` marks
        // that terminal state so `dequeuePending` can exclude it.
        migrator.registerMigration("v2_offline_queue_attempts") { db in
            try db.alter(table: "offline_queue") { t in
                t.add(column: "attempts", .integer).notNull().defaults(to: 0)
                t.add(column: "abandoned", .boolean).notNull().defaults(to: false)
            }

            // `markComplete` previously left completed rows in place
            // (`completed_at` set, never deleted); it now deletes them
            // outright. Sweep any rows a prior app version left behind so
            // they don't sit excluded-but-never-removed forever.
            try db.execute(sql: "DELETE FROM offline_queue WHERE completed_at IS NOT NULL")
        }

        try migrator.migrate(db)
    }
}
