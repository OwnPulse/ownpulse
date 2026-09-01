// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
@testable import OwnPulse

/// Wraps a real `OfflineQueue` (backed by an in-memory GRDB database) so
/// tests can inject enqueue failures without reimplementing persistence.
/// Used to prove the anchor-integrity invariant: if a GRDB `enqueue` call
/// throws, `SyncEngine` must not advance the persisted anchor past a page
/// containing a batch that was neither uploaded nor durably queued.
final class MockOfflineQueue: OfflineQueueProtocol, @unchecked Sendable {
    private let wrapped: OfflineQueue
    private let lock = NSLock()
    private var _enqueueShouldFail = false
    private var _enqueueFailAtCall: Int?
    private var _enqueueCallCount = 0
    private var _recordFailedAttemptCallCount = 0

    /// When `true`, every `enqueue` call fails.
    var enqueueShouldFail: Bool {
        get { lock.lock(); defer { lock.unlock() }; return _enqueueShouldFail }
        set { lock.lock(); _enqueueShouldFail = newValue; lock.unlock() }
    }

    /// When set, only the Nth `enqueue` call (1-indexed) fails — the rest
    /// succeed. Used to reproduce "batch 1 acked, batch 2's enqueue fails,
    /// batches 3+ enqueue fine" within a single multi-batch page.
    var enqueueFailAtCall: Int? {
        get { lock.lock(); defer { lock.unlock() }; return _enqueueFailAtCall }
        set { lock.lock(); _enqueueFailAtCall = newValue; lock.unlock() }
    }

    var enqueueCallCount: Int {
        lock.lock(); defer { lock.unlock() }; return _enqueueCallCount
    }

    var recordFailedAttemptCallCount: Int {
        lock.lock(); defer { lock.unlock() }; return _recordFailedAttemptCallCount
    }

    init(databaseManager: DatabaseManager, currentBuild: String = "test-build") {
        self.wrapped = OfflineQueue(databaseManager: databaseManager, currentBuild: currentBuild)
    }

    func enqueue(_ records: HealthKitBulkInsert) throws {
        lock.lock()
        _enqueueCallCount += 1
        let callNumber = _enqueueCallCount
        let shouldFail = _enqueueShouldFail || _enqueueFailAtCall == callNumber
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

    @discardableResult
    func recordFailedAttempt(id: Int64) throws -> Bool {
        lock.lock()
        _recordFailedAttemptCallCount += 1
        lock.unlock()
        return try wrapped.recordFailedAttempt(id: id)
    }

    func abandonedCount() throws -> Int {
        try wrapped.abandonedCount()
    }

    func retryAbandoned() throws {
        try wrapped.retryAbandoned()
    }
}
