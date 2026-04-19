// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

/// Bridges `HKObserverQuery` events into `SyncEngine.sync()` calls with
/// trailing-edge debounce. HealthKit fires observers eagerly — during a
/// workout, a heart-rate stream can yield dozens of updates per minute — so
/// we coalesce bursts into a single sync.
///
/// Each instance owns at most one subscription at a time. Calling
/// `start(authenticatedOnly:)` while already running is a no-op; callers stop
/// the current subscription via `stop()` before starting a new one (e.g. on
/// logout).
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
    func stop() {
        subscriptionTask?.cancel()
        subscriptionTask = nil
        pendingSyncTask?.cancel()
        pendingSyncTask = nil
        lastEventAt = nil
    }

    // MARK: - Internal

    private func onObserverFired() async {
        lastEventAt = clock()

        // If a sync is already pending, the existing debounce task will pick
        // up the updated `lastEventAt` and wait out a fresh debounce window.
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

        await syncEngine.sync()
    }
}
