// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import GRDB

final class DatabaseManager: Sendable {
    let dbQueue: DatabaseQueue

    init(inMemory: Bool = false) {
        do {
            // GRDB's default `busyMode` is `.immediateError` — zero retry,
            // so any other connection merely holding a write lock at the
            // exact moment we open (or migrate) causes an instant
            // `SQLITE_BUSY` here. `.timeout` retries for a bit instead of
            // failing immediately, so a transient lock (another in-process
            // connection to the same file mid-write) resolves on its own
            // rather than crashing the app.
            var configuration = Configuration()
            configuration.busyMode = .timeout(5)

            if inMemory {
                dbQueue = try DatabaseQueue(configuration: configuration)
            } else {
                let url = try FileManager.default
                    .url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                    .appendingPathComponent("ownpulse.sqlite")
                dbQueue = try DatabaseQueue(path: url.path, configuration: configuration)
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
