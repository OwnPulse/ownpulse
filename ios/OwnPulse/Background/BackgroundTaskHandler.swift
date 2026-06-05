// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import BackgroundTasks

/// The sync work a background refresh triggers. `SyncEngine` (an `actor`)
/// satisfies this; tests pass a mock. Crossing this seam is always `await`,
/// so no caller ever assumes a particular executor.
protocol BackgroundSyncing: Sendable {
    func sync() async
}

extension SyncEngine: BackgroundSyncing {}

/// The slice of `BGTask` the handler actually drives: register an expiration
/// handler and report completion. Abstracting it lets us unit-test the
/// completion lifecycle without a real `BGAppRefreshTask` (which cannot be
/// constructed in a test host).
///
/// Both members are deliberately NOT `@MainActor`. `BGTaskScheduler` invokes
/// the launch handler — and fires the expiration handler — on a background
/// dispatch queue, so anything reachable from here must be safe off the main
/// actor. Marking it `@MainActor` would reintroduce the isolation-assert
/// crash this file exists to prevent.
///
/// This protocol intentionally does NOT refine `Sendable`. `BGTask` is a
/// non-`final` imported ObjC class and is not `Sendable`, so a checked
/// conformance to a `Sendable`-refining protocol is a hard error in Swift 6
/// ("conformance to 'Sendable' must occur in the same source file"). The one
/// place a task crosses an isolation boundary — the launch handler in
/// `OwnPulseApp` — already gates it with `nonisolated(unsafe)`, and the
/// finish/expiration race is mediated by `CompletionGuard` (`@unchecked
/// Sendable`), so the task value itself never needs to be `Sendable`.
protocol BackgroundTaskCompleting: AnyObject {
    /// The closure iOS calls when our execution time is about to expire. It
    /// fires on a background queue.
    var expirationHandler: (() -> Void)? { get set }
    /// Report completion to the scheduler. Must be called exactly once.
    func setTaskCompleted(success: Bool)
}

extension BGTask: BackgroundTaskCompleting {}

enum BackgroundTaskHandler {
    /// Runs a background sync and reports completion exactly once.
    ///
    /// Executor contract: this function makes no synchronous main-actor
    /// access. `task` is a non-`@MainActor` `BackgroundTaskCompleting`, the
    /// sync hop is `await` across the actor boundary, and the expiration
    /// handler only cancels in-flight work and reports completion — none of
    /// which touches main-actor state. It is therefore safe to invoke
    /// directly from the background queue `BGTaskScheduler` runs the launch
    /// handler on, and from a nonisolated test context.
    ///
    /// Completion-exactly-once: a single `CompletionGuard` arbitrates between
    /// the normal finish path and the expiration handler. Whichever fires
    /// first wins; the other is a no-op. Expiration also cancels the wrapping
    /// task so the engine can wind down — the anchor store and offline queue
    /// resume from the same point on the next foreground sync.
    static func handleSync(
        task: BackgroundTaskCompleting,
        syncEngine: BackgroundSyncing
    ) async {
        let guardian = CompletionGuard(task: task)

        // Wrap the sync in a child task so the expiration handler can cancel
        // it without blocking the background queue it fires on.
        let work = Task {
            await syncEngine.sync()
        }

        task.expirationHandler = {
            // Fires on a background queue. Cancel the in-flight sync and
            // report failure — but only if the normal path hasn't already
            // completed the task.
            work.cancel()
            guardian.complete(success: false)
        }

        await work.value

        // If the expiration handler already reported completion, this is a
        // no-op; skip rescheduling so we don't queue a fresh request on a run
        // the system just cut short.
        guard guardian.complete(success: true) else { return }

        // Schedule the next refresh after a clean finish.
        SyncScheduler().scheduleNextSync()
    }
}

/// Ensures `setTaskCompleted(success:)` is called exactly once across the
/// normal-finish and expiration paths, which can race on different queues.
/// Returns `true` from `complete` only for the call that actually reported
/// completion, so the caller knows whether it "won".
private final class CompletionGuard: @unchecked Sendable {
    private let lock = NSLock()
    private var done = false
    private let task: BackgroundTaskCompleting

    init(task: BackgroundTaskCompleting) {
        self.task = task
    }

    /// Reports completion to the task if it hasn't been reported yet.
    /// - Returns: `true` if this call reported completion, `false` if a prior
    ///   call already did.
    @discardableResult
    func complete(success: Bool) -> Bool {
        lock.lock()
        if done {
            lock.unlock()
            return false
        }
        done = true
        lock.unlock()
        task.setTaskCompleted(success: success)
        return true
    }
}
