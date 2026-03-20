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
            fatalError("Database setup failed: \(error)")
        }
    }
}
