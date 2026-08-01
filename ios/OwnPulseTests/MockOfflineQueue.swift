// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
@testable import OwnPulse

/// Wraps a real `OfflineQueue` (backed by an in-memory GRDB database) so
/// tests can inject enqueue failures without reimplementing persistence.
/// Used to prove the anchor-integrity invariant: if a GRDB `enqueue` call
/// throws, `SyncEngine` must not advance `lastAckedAnchor` past the batch
/// that was neither uploaded nor durably queued.
final class MockOfflineQueue: OfflineQueueProtocol, @unchecked Sendable {
    private let wrapped: OfflineQueue
    private let lock = NSLock()
    private var _enqueueShouldFail = false
    private var _enqueueCallCount = 0
    private var _recordFailedAttemptCallCount = 0

    var enqueueShouldFail: Bool {
        get { lock.lock(); defer { lock.unlock() }; return _enqueueShouldFail }
        set { lock.lock(); _enqueueShouldFail = newValue; lock.unlock() }
    }

    var enqueueCallCount: Int {
        lock.lock(); defer { lock.unlock() }; return _enqueueCallCount
    }

    var recordFailedAttemptCallCount: Int {
        lock.lock(); defer { lock.unlock() }; return _recordFailedAttemptCallCount
    }

    init(databaseManager: DatabaseManager) {
        self.wrapped = OfflineQueue(databaseManager: databaseManager)
    }

    func enqueue(_ records: HealthKitBulkInsert) throws {
        lock.lock()
        _enqueueCallCount += 1
        let shouldFail = _enqueueShouldFail
        lock.unlock()

        if shouldFail {
            throw NSError(
                domain: "test.offline-queue",
                code: 1,
                userInfo: [NSLocalizedDescriptionKey: "simulated GRDB enqueue failure"]
            )
        }
        try wrapped.enqueue(records)
    }

    func dequeuePending() throws -> [(id: Int64, insert: HealthKitBulkInsert)] {
        try wrapped.dequeuePending()
    }

    func markComplete(id: Int64) throws {
        try wrapped.markComplete(id: id)
    }

    func recordFailedAttempt(id: Int64) throws {
        lock.lock()
        _recordFailedAttemptCallCount += 1
        lock.unlock()
        try wrapped.recordFailedAttempt(id: id)
    }
}
