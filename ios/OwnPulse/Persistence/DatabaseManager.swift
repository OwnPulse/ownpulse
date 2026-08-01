// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import GRDB

final class DatabaseManager: Sendable {
    let dbQueue: DatabaseQueue

    init(inMemory: Bool = false) {
        do {
            if inMemory {
                dbQueue = try DatabaseQueue()
            } else {
                let url = try FileManager.default
                    .url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                    .appendingPathComponent("ownpulse.sqlite")
                dbQueue = try DatabaseQueue(path: url.path)
            }
            try Migrations.run(dbQueue)
        } catch {
            // A personal-health-data app that can't open its own database
            // should fail loudly rather than run in some degraded/undefined
            // state — keep the fatalError. But log only the error's TYPE:
            // GRDB's `DatabaseError.description` can embed the failing SQL
            // statement (and, if `publicStatementArguments` were ever
            // enabled, bound values) and SQLite's own message can include
            // the on-disk file path — none of that belongs in a crash
            // report.
            fatalError("Database setup failed: \(type(of: error))")
        }
    }
}
