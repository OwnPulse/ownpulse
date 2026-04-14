// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Observation

enum SyncTypeState: Sendable {
    case pending
    case syncing
    case synced
    case skipped
    case failed
    case never
}

struct TypeSyncStatus: Sendable {
    let recordType: String
    let displayName: String
    var status: SyncTypeState
    var lastSyncTime: Date?
    var recordsSynced: Int
    var error: String?
}

@Observable
@MainActor
final class SyncProgress {
    var typeStatuses: [String: TypeSyncStatus] = [:]
    var currentType: String?
    var totalTypes: Int = 0
    var completedTypes: Int = 0

    func reset(types: [(recordType: String, displayName: String)], timestamps: [String: Date]) {
        totalTypes = types.count
        completedTypes = 0
        currentType = nil
        typeStatuses = [:]
        for t in types {
            let lastSync = timestamps[t.recordType]
            typeStatuses[t.recordType] = TypeSyncStatus(
                recordType: t.recordType,
                displayName: t.displayName,
                status: lastSync != nil ? .synced : .never,
                lastSyncTime: lastSync,
                recordsSynced: 0,
                error: nil
            )
        }
    }

    func markSyncing(_ recordType: String) {
        currentType = recordType
        typeStatuses[recordType]?.status = .syncing
        typeStatuses[recordType]?.error = nil
    }

    func markSynced(_ recordType: String, count: Int, at date: Date) {
        typeStatuses[recordType]?.status = .synced
        typeStatuses[recordType]?.recordsSynced = count
        typeStatuses[recordType]?.lastSyncTime = date
        completedTypes += 1
    }

    func markSkipped(_ recordType: String) {
        typeStatuses[recordType]?.status = .skipped
        completedTypes += 1
    }

    func markFailed(_ recordType: String, error: String) {
        typeStatuses[recordType]?.status = .failed
        typeStatuses[recordType]?.error = error
        completedTypes += 1
    }

    func finish() {
        currentType = nil
    }
}
