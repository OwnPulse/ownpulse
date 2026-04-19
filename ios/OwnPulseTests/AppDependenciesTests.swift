// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import BackgroundTasks
import Foundation
import SwiftUI
import Testing
@testable import OwnPulse

@Suite("AppDependencies — auto-sync lifecycle wiring")
@MainActor
struct AppDependenciesTests {
    // MARK: - Helpers

    /// Builds an AppDependencies with the explicit test doubles the suite
    /// needs. Returns the container plus the doubles so tests can observe
    /// scheduler/observer/background-delivery side effects.
    private func make() -> (
        deps: AppDependencies,
        provider: MockHealthKitProvider,
        submitter: RecordingSubmitter
    ) {
        let keychain = MockKeychainService()
        let network = MockNetworkClient()
        network.requestHandler = { method, path, _ in
            if method == "GET" && path == Endpoints.healthKitWriteQueue {
                return [HealthKitWriteQueueItem]()
            }
            return [] as [AuthMethod]
        }
        network.requestNoContentHandler = { _, _, _ in /* no-op */ }

        let provider = MockHealthKitProvider()
        let submitter = RecordingSubmitter()
        let scheduler = SyncScheduler(submitter: submitter)

        let deps = AppDependencies(
            keychainService: keychain,
            networkClient: network,
            healthKitProvider: provider,
            syncScheduler: scheduler
        )
        return (deps, provider, submitter)
    }

    // MARK: - Fix #1: logout wiring

    @Test("logout stops the coordinator and disables background delivery")
    func logoutTearsDownAutoSync() async throws {
        let (deps, provider, _) = make()

        // Bring the app into the "logged-in with auto-sync running" state.
        // processCallback is the cleanest path — it sets both tokens and
        // fires onLoginSuccess which bootstraps.
        let url = URL(string: "ownpulse://auth#token=jwt&refresh_token=refresh")!
        try await deps.authService.processCallback(url: url)

        try await eventually(timeout: 2.0) {
            provider.backgroundDeliveryCallCount >= 1
        }
        try await eventually(timeout: 2.0) {
            provider.observerStartCount >= 1
        }

        // Logout triggers the teardown hook.
        await deps.authService.logout()

        #expect(provider.disableBackgroundDeliveryCallCount >= 1)
        #expect(provider.backgroundDeliveryDisabled == true)
        #expect(deps.authService.isAuthenticated == false)
    }

    // MARK: - Fix #2: first-time login bootstraps BGAppRefresh + background delivery

    @Test("first-time login schedules BGAppRefresh AND enables background delivery")
    func firstTimeLoginBootstrapsEverything() async throws {
        let (deps, provider, submitter) = make()

        // Pre-condition: a fresh install has no tokens and isn't authed, so
        // bootstrapAutoSync() early-returns. Before the fix, the login hook
        // only started the coordinator and ran a sync — it never scheduled
        // BGAppRefresh or enabled background delivery.
        #expect(deps.authService.isAuthenticated == false)
        #expect(submitter.requests.count == 0)
        #expect(provider.backgroundDeliveryCallCount == 0)

        // Simulate first-time Google OAuth callback.
        let url = URL(string: "ownpulse://auth#token=jwt&refresh_token=refresh")!
        try await deps.authService.processCallback(url: url)

        try await eventually(timeout: 2.0) {
            submitter.requests.count >= 1
        }
        try await eventually(timeout: 2.0) {
            provider.backgroundDeliveryCallCount >= 1
        }
        #expect(provider.observerStartCount >= 1)

        // The request going to the submitter must be a BGAppRefresh with the
        // OwnPulse identifier.
        let request = submitter.requests.first
        #expect(request?.identifier == SyncScheduler.taskIdentifier)
        #expect(request is BGAppRefreshTaskRequest)
    }

    // MARK: - Fix #5: scene-phase policy

    @Test("scene phase .active while unauthenticated does NOT trigger a sync")
    func unauthedActiveIsNoOp() {
        let (deps, _, _) = make()
        #expect(deps.authService.isAuthenticated == false)

        let triggered = deps.handleScenePhase(.active)
        #expect(triggered == false)
    }

    @Test("scene phase .background / .inactive while authenticated does NOT trigger a sync")
    func nonActivePhasesAreNoOp() async throws {
        let (deps, _, _) = make()
        let url = URL(string: "ownpulse://auth#token=jwt&refresh_token=refresh")!
        try await deps.authService.processCallback(url: url)
        #expect(deps.authService.isAuthenticated == true)

        #expect(deps.handleScenePhase(.inactive) == false)
        #expect(deps.handleScenePhase(.background) == false)
    }

    @Test("scene phase .active while authenticated triggers a sync")
    func authedActiveTriggersSync() async throws {
        let (deps, _, _) = make()
        let url = URL(string: "ownpulse://auth#token=jwt&refresh_token=refresh")!
        try await deps.authService.processCallback(url: url)

        #expect(deps.handleScenePhase(.active) == true)
    }

    @Test("rapid scene-phase active flips coalesce via the sync engine's re-entrancy guard")
    func rapidActiveFlipsCoalesce() async throws {
        let (deps, _, _) = make()
        let url = URL(string: "ownpulse://auth#token=jwt&refresh_token=refresh")!
        try await deps.authService.processCallback(url: url)

        // Fire three in quick succession. All three return true because the
        // policy doesn't try to coalesce at the policy layer — it's the
        // SyncEngine's `guard !_isSyncing` that drops overlapping calls.
        // We just assert none of them trap/panic.
        #expect(deps.handleScenePhase(.active) == true)
        #expect(deps.handleScenePhase(.active) == true)
        #expect(deps.handleScenePhase(.active) == true)
    }
}

// MARK: - Test doubles

/// Records submitted `BGTaskRequest` instances so tests can observe what the
/// real `SyncScheduler` was asked to schedule, without actually handing the
/// request off to `BGTaskScheduler.shared` (which raises in the unit-test
/// host without a valid entitlement).
///
/// File-scoped so the similarly-named double in `SyncSchedulerTests.swift`
/// doesn't conflict — each file gets its own `private` type.
fileprivate final class RecordingSubmitter: BackgroundTaskSubmitter, @unchecked Sendable {
    private let lock = NSLock()
    private var _requests: [BGTaskRequest] = []

    var requests: [BGTaskRequest] {
        lock.lock(); defer { lock.unlock() }
        return _requests
    }

    func submit(_ request: BGTaskRequest) throws {
        lock.lock()
        _requests.append(request)
        lock.unlock()
    }
}

/// Polls `condition` up to `timeout` seconds, sleeping 20ms between checks.
/// Records an Issue if the condition never becomes true.
@MainActor
private func eventually(
    timeout: TimeInterval,
    _ condition: @MainActor () async -> Bool
) async throws {
    let deadline = Date().addingTimeInterval(timeout)
    while Date() < deadline {
        if await condition() { return }
        try await Task.sleep(nanoseconds: 20_000_000)
    }
    Issue.record("Condition never became true within \(timeout)s")
}
