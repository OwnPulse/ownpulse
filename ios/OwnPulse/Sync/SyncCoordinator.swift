// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

/// Bridges `HKObserverQuery` events into `SyncEngine.sync()` calls with
/// trailing-edge debounce. HealthKit fires observers eagerly — during a
/// workout, a heart-rate stream can yield dozens of updates per minute — so
/// we coalesce bursts into a single sync.
///
/// Each instance owns at most one subscription at a time. Calling
/// `start()` while already running is a no-op; callers stop the current
/// subscription via `stop()` before starting a new one (e.g. on logout).
///
/// Events that arrive while a sync is in flight are **not** lost — the
/// coordinator records that a follow-up run is needed and re-enters the
/// debounce pipeline after the current sync returns. Without this, samples
/// written during the tail of a long sync (a workout completing mid-upload)
/// would wait for the next BGAppRefresh window before syncing.
actor SyncCoordinator {
    /// Trailing-edge debounce window. HealthKit bursts during workouts
    /// regularly exceed 10 samples/second; 3s gives us a comfortable quiet
    /// window without making users wait noticeably for their data.
    static let defaultDebounceSeconds: Double = 3.0

    private let healthKitProvider: HealthKitProviderProtocol
    private let syncEngine: SyncEngine
    private let debounceSeconds: Double
    private let clock: @Sendable () -> Date
    private let sleep: @Sendable (Double) async throws -> Void

    private var subscriptionTask: Task<Void, Never>?
    private var pendingSyncTask: Task<Void, Never>?
    private var lastEventAt: Date?

    /// Tracks observer events that arrive while a sync is already running.
    /// When `runDebouncedSync` returns from `await syncEngine.sync()`, it
    /// checks this flag and re-enters the pipeline to pick up anything that
    /// arrived during the in-flight sync. Without this, events that fire
    /// between "sync started" and "sync finished" would be silently dropped
    /// — the engine's `guard !_isSyncing else { return }` would eat the
    /// follow-up call.
    private var syncInFlight = false
    private var needsAnotherRunAfterCurrent = false

    init(
        healthKitProvider: HealthKitProviderProtocol,
        syncEngine: SyncEngine,
        debounceSeconds: Double = SyncCoordinator.defaultDebounceSeconds,
        clock: @escaping @Sendable () -> Date = { Date() },
        sleep: @escaping @Sendable (Double) async throws -> Void = { seconds in
            try await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
        }
    ) {
        self.healthKitProvider = healthKitProvider
        self.syncEngine = syncEngine
        self.debounceSeconds = debounceSeconds
        self.clock = clock
        self.sleep = sleep
    }

    /// Begin listening for HealthKit updates. Each event schedules (or
    /// re-schedules) a debounced sync.
    func start() {
        guard subscriptionTask == nil else { return }

        let stream = healthKitProvider.observeSampleUpdates()
        subscriptionTask = Task { [weak self] in
            for await _ in stream {
                guard let self else { return }
                await self.onObserverFired()
            }
        }
    }

    /// Stop the observer subscription and cancel any pending debounced sync.
    /// After stop() the coordinator is inert until start() is called again.
    /// Called from the logout hook to ensure no further HealthKit-driven
    /// sync attempts fire against an expired token.
    func stop() {
        subscriptionTask?.cancel()
        subscriptionTask = nil
        pendingSyncTask?.cancel()
        pendingSyncTask = nil
        lastEventAt = nil
        needsAnotherRunAfterCurrent = false
        // Note: we do NOT reset syncInFlight — if a sync is actively running
        // on the SyncEngine actor we let it finish naturally. Its completion
        // will observe that the follow-up flag is clear and stop.
    }

    // MARK: - Internal

    private func onObserverFired() async {
        lastEventAt = clock()

        // If a sync is already actively running (past the debounce, on the
        // engine), mark that we need another run after it returns. Without
        // this flag, fires during the in-flight sync would be dropped by
        // the engine's re-entrancy guard and never re-scheduled.
        if syncInFlight {
            needsAnotherRunAfterCurrent = true
            return
        }

        // If a debounced task is already waiting, its loop will pick up the
        // updated `lastEventAt` and reset its quiet window.
        if pendingSyncTask != nil { return }

        pendingSyncTask = Task { [weak self] in
            await self?.runDebouncedSync()
        }
    }

    private func runDebouncedSync() async {
        // Trailing-edge debounce: keep sleeping while new events keep
        // arriving. Only fire the sync once the quiet window elapses without
        // new events.
        while let last = lastEventAt {
            let remaining = debounceSeconds - clock().timeIntervalSince(last)
            if remaining <= 0 { break }
            do {
                try await sleep(remaining)
            } catch {
                // Task cancelled — abandon the pending sync.
                pendingSyncTask = nil
                return
            }
        }

        lastEventAt = nil
        pendingSyncTask = nil
        syncInFlight = true

        await syncEngine.sync()

        syncInFlight = false

        // If events arrived during the sync, run another debounce cycle to
        // pick them up. We stamp `lastEventAt` to now so the next cycle
        // debounces from this point, matching trailing-edge semantics.
        if needsAnotherRunAfterCurrent {
            needsAnotherRunAfterCurrent = false
            lastEventAt = clock()
            pendingSyncTask = Task { [weak self] in
                await self?.runDebouncedSync()
            }
        }
    }
}
