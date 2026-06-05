// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

/// These tests deliberately run in a NON-`@MainActor` (nonisolated) context —
/// the suite struct has no `@MainActor` annotation and the test functions are
/// `nonisolated`. That mirrors how `BGTaskScheduler` invokes the launch handler
/// on a background dispatch queue. If `BackgroundTaskHandler.handleSync` (or the
/// types it touches) synchronously asserted main-actor isolation, these tests
/// would trap exactly the way the production crash did
/// (`_swift_task_checkIsolatedSwift` / `dispatch_assert_queue`).
@Suite("BackgroundTaskHandler")
struct BackgroundTaskHandlerTests {
    // MARK: - Success path

    @Test("handleSync runs the sync and reports completion success exactly once")
    func successPathCompletesOnce() async {
        let engine = MockBackgroundSyncEngine()
        let task = MockBackgroundTask()

        await BackgroundTaskHandler.handleSync(task: task, syncEngine: engine)

        #expect(await engine.syncCallCount == 1)
        #expect(task.completionResults == [true])
        #expect(task.setTaskCompletedCallCount == 1)
        // The handler installs an expiration handler before running the work.
        #expect(task.expirationHandlerWasSet)
    }

    // MARK: - Expiration path

    @Test("expiration before completion reports failure and cancels the sync")
    func expirationReportsFailureOnce() async {
        // Hold the sync open until expiration has had a chance to win.
        let engine = MockBackgroundSyncEngine(blockUntilCancelled: true)
        let task = MockBackgroundTask()

        // Fire expiration the moment the handler installs it — before the sync
        // finishes. This is the race the CompletionGuard arbitrates.
        task.onExpirationHandlerSet = { handler in
            handler()
        }

        await BackgroundTaskHandler.handleSync(task: task, syncEngine: engine)

        // Exactly one completion, and it is the expiration's failure result —
        // the later success call must be a no-op.
        #expect(task.setTaskCompletedCallCount == 1)
        #expect(task.completionResults == [false])
        #expect(await engine.wasCancelled)
    }

    // MARK: - Race: normal finish wins, late expiration is a no-op

    @Test("late expiration after a clean finish does not complete the task twice")
    func lateExpirationIsNoOp() async {
        let engine = MockBackgroundSyncEngine()
        let task = MockBackgroundTask()

        await BackgroundTaskHandler.handleSync(task: task, syncEngine: engine)

        // Simulate iOS firing the expiration handler AFTER we already reported
        // success. It must be a no-op — setTaskCompleted called exactly once.
        task.fireExpirationHandler()

        #expect(task.setTaskCompletedCallCount == 1)
        #expect(task.completionResults == [true])
    }

    // MARK: - Executor safety

    @Test("handleSync does not assert main-actor isolation off the main actor")
    func runsOffMainActorWithoutTrapping() async {
        // Explicitly detach onto a non-main executor and confirm completion.
        // A synchronous main-actor assert anywhere in the path would crash the
        // test process here rather than fail an expectation.
        let engine = MockBackgroundSyncEngine()
        let task = MockBackgroundTask()

        await Task.detached {
            #expect(!Thread.isMainThread)
            await BackgroundTaskHandler.handleSync(task: task, syncEngine: engine)
        }.value

        #expect(task.completionResults == [true])
    }
}

// MARK: - Test doubles

/// Mock `BackgroundSyncing`. Records call count and supports holding the sync
/// open until cancellation so the expiration race can be exercised
/// deterministically.
private actor MockBackgroundSyncEngine: BackgroundSyncing {
    private(set) var syncCallCount = 0
    private(set) var wasCancelled = false
    private let blockUntilCancelled: Bool

    init(blockUntilCancelled: Bool = false) {
        self.blockUntilCancelled = blockUntilCancelled
    }

    func sync() async {
        syncCallCount += 1
        guard blockUntilCancelled else { return }
        // Spin cooperatively until the wrapping task is cancelled by the
        // expiration handler. Bounded so a logic error can't hang CI forever.
        for _ in 0..<10_000 {
            if Task.isCancelled {
                wasCancelled = true
                return
            }
            await Task.yield()
        }
    }
}

/// Mock `BackgroundTaskCompleting`. Records every `setTaskCompleted` result and
/// exposes the installed expiration handler so tests can fire it on demand.
private final class MockBackgroundTask: BackgroundTaskCompleting, @unchecked Sendable {
    private let lock = NSLock()
    private var _expirationHandler: (() -> Void)?
    private var _completionResults: [Bool] = []

    /// Invoked synchronously when the handler assigns `expirationHandler`.
    /// Lets a test fire expiration the instant it is installed.
    var onExpirationHandlerSet: ((@escaping () -> Void) -> Void)?

    var expirationHandler: (() -> Void)? {
        get {
            lock.lock(); defer { lock.unlock() }
            return _expirationHandler
        }
        set {
            lock.lock()
            _expirationHandler = newValue
            let hook = onExpirationHandlerSet
            lock.unlock()
            if let newValue, let hook {
                hook(newValue)
            }
        }
    }

    func setTaskCompleted(success: Bool) {
        lock.lock()
        _completionResults.append(success)
        lock.unlock()
    }

    // MARK: Test inspection

    var completionResults: [Bool] {
        lock.lock(); defer { lock.unlock() }
        return _completionResults
    }

    var setTaskCompletedCallCount: Int {
        lock.lock(); defer { lock.unlock() }
        return _completionResults.count
    }

    var expirationHandlerWasSet: Bool {
        lock.lock(); defer { lock.unlock() }
        return _expirationHandler != nil
    }

    func fireExpirationHandler() {
        lock.lock()
        let handler = _expirationHandler
        lock.unlock()
        handler?()
    }
}
