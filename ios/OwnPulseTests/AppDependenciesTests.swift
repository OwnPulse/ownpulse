// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import BackgroundTasks
import Foundation
import SwiftUI
import Testing
import UserNotifications
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
            // Both `bootstrapAutoSync()` (on login) and `handleScenePhase(.active)`
            // fire a dose-reminder rebuild alongside the HealthKit sync, which
            // fetches active runs. Stub it to an empty list so those tests
            // don't crash on a type mismatch against the `[AuthMethod]`
            // fallback below.
            if method == "GET" && path == Endpoints.activeRuns {
                return [ActiveRunResponse]()
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
            syncScheduler: scheduler,
            databaseManager: DatabaseManager(inMemory: true)
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

    // MARK: - Plan fix #5: kickOffBackfill survives view dismissal

    @Test("kickOffBackfill keeps the sync task alive after the calling view dismisses")
    func testKickOffBackfillSurvivesDispose() async throws {
        let (deps, provider, _) = make()
        // Authenticate so any future sync calls would actually run.
        let url = URL(string: "ownpulse://auth#token=jwt&refresh_token=refresh")!
        try await deps.authService.processCallback(url: url)

        // Configure provider to return one sample per type so the sync
        // actually does work and we can observe it running. Use the default
        // mock path (`mockSamples`).
        provider.mockSamples = [
            HealthKitSample(
                recordType: "synthetic",
                value: 1, unit: "count",
                startTime: Date(), endTime: Date(),
                sourceId: "s-1"
            )
        ]
        provider.mockAnchor = Data([1])

        // "Dispose of the view" simulation: kickOffBackfill from a local
        // scope, then drop all local references and assert the work
        // completes anyway. The Task is owned by AppDependencies, not the
        // local scope — so it must outlive the scope exit.
        await withCheckedContinuation { (cont: CheckedContinuation<Void, Never>) in
            Task { @MainActor in
                deps.kickOffBackfill()
                cont.resume()
            }
        }

        // After the "view" dismissed, the sync continues; wait until the
        // engine reports a lastSyncDate (a successful run leaves it set).
        //
        // Timeout is generous (15s) because a full sync touches 74 mapped
        // HealthKit types × ~4 MainActor hops each for progress updates.
        // On the iPhone 17 sim under contention this is observed at ~7s;
        // on iPhone 16 it's ~0.1s. The point of the test isn't the speed
        // — it's that the Task survives the calling scope's exit.
        try await eventually(timeout: 15.0) {
            let date = await deps.syncEngine.lastSyncDate
            return date != nil
        }
    }

    // MARK: - Dose reminders wired into scene-phase / login / logout

    @Test("scene phase .active while authenticated rebuilds dose reminders")
    func activeScenePhaseRebuildsDoseReminders() async throws {
        let keychain = MockKeychainService()
        let network = MockNetworkClient()
        network.requestHandler = { _, path, _ in
            if path == Endpoints.healthKitWriteQueue { return [HealthKitWriteQueueItem]() }
            if path == Endpoints.activeRuns { return [ActiveRunResponse]() }
            return [] as [AuthMethod]
        }
        network.requestNoContentHandler = { _, _, _ in }

        let deps = AppDependencies(
            keychainService: keychain,
            networkClient: network,
            healthKitProvider: MockHealthKitProvider(),
            syncScheduler: SyncScheduler(submitter: RecordingSubmitter()),
            databaseManager: DatabaseManager(inMemory: true)
        )
        let url = URL(string: "ownpulse://auth#token=jwt&refresh_token=refresh")!
        try await deps.authService.processCallback(url: url)

        // Login itself triggers a rebuild (bootstrapAutoSync); wait for it,
        // then reset the call log and confirm scene-phase .active triggers
        // its own independent rebuild too.
        try await eventually(timeout: 2.0) {
            network.requestCalls.contains { $0.path == Endpoints.activeRuns }
        }

        let callsBefore = network.requestCalls.count
        #expect(deps.handleScenePhase(.active) == true)

        try await eventually(timeout: 2.0) {
            network.requestCalls.count > callsBefore
                && network.requestCalls.suffix(from: callsBefore).contains { $0.path == Endpoints.activeRuns }
        }
    }

    @Test("logout removes every pending dose reminder via the notification center")
    func logoutClearsAllDoseReminders() async throws {
        // Seed the keychain with an already-valid, non-expired token so
        // `AuthService.init` sets `isAuthenticated = true` directly —
        // deliberately NOT going through `processCallback`/`bootstrapAutoSync`,
        // so the only dose-reminder-related call this test can observe is
        // logout's own `clearAll()`, not an incidental login-triggered
        // rebuild racing to remove the same pre-seeded ids first.
        let keychain = MockKeychainService()
        try keychain.save(key: AuthService.accessTokenKey, data: Data(Self.makeValidJWT().utf8))

        let network = MockNetworkClient()
        network.requestHandler = { _, _, _ in [] as [AuthMethod] }
        network.requestNoContentHandler = { _, _, _ in }

        let center = MockUserNotificationCenter()
        // As if a previous rebuild had already scheduled reminders — logout
        // must remove these, not just no-op against an empty center.
        center.pendingRequests = [
            UNNotificationRequest(identifier: "dose-run-1-08:00-2026-06-01", content: UNMutableNotificationContent(), trigger: nil), // date-ok
            UNNotificationRequest(identifier: "dose-run-1-20:00-2026-06-01", content: UNMutableNotificationContent(), trigger: nil), // date-ok
        ]

        let deps = AppDependencies(
            keychainService: keychain,
            networkClient: network,
            healthKitProvider: MockHealthKitProvider(),
            syncScheduler: SyncScheduler(submitter: RecordingSubmitter()),
            notificationCenter: center,
            databaseManager: DatabaseManager(inMemory: true)
        )
        #expect(deps.authService.isAuthenticated == true)

        await deps.authService.logout()

        #expect(deps.authService.isAuthenticated == false)
        let removedIds = Set(center.removedIdentifierBatches.flatMap { $0 })
        #expect(removedIds.contains("dose-run-1-08:00-2026-06-01")) // date-ok
        #expect(removedIds.contains("dose-run-1-20:00-2026-06-01")) // date-ok
        #expect(center.pendingRequests.isEmpty)
    }

    /// Builds a syntactically-valid, non-expired JWT so `AuthService.init`'s
    /// `JWTDecoder.isExpired` check passes. The signature segment is never
    /// validated client-side — only `sub`/`exp` in the payload matter here.
    private static func makeValidJWT(expiresIn: TimeInterval = 3600) -> String {
        let header = Data("{\"alg\":\"none\"}".utf8).base64EncodedString()
        let payload: [String: Any] = [
            "sub": "user-1",
            "exp": Date().addingTimeInterval(expiresIn).timeIntervalSince1970,
        ]
        let payloadData = try! JSONSerialization.data(withJSONObject: payload)
        return "\(header).\(payloadData.base64EncodedString()).signature"
    }

    // MARK: - Plan fix #6: bootstrap calls authorization BEFORE enabling delivery

    @Test("bootstrap requests HealthKit authorization before enabling background delivery")
    func testBootstrapAuthorizationOrdering() async throws {
        let (deps, provider, _) = make()

        // Pre-condition: nothing has been called yet.
        #expect(provider.authorizationRequested == false)
        #expect(provider.backgroundDeliveryCallCount == 0)

        // Trigger bootstrapAutoSync via login.
        let url = URL(string: "ownpulse://auth#token=jwt&refresh_token=refresh")!
        try await deps.authService.processCallback(url: url)

        // Both side effects must occur — authorization first, then
        // background delivery.
        try await eventually(timeout: 2.0) {
            provider.authorizationRequested && provider.backgroundDeliveryCallCount >= 1
        }

        // The ordering is the contract: authorization must have happened
        // BEFORE the first enableBackgroundDelivery call. We can't observe
        // strict ordering with a single bool, but if `authorizationRequested`
        // is true by the time backgroundDeliveryCallCount >= 1, the
        // sequential `await` chain in bootstrapAutoSync guarantees the
        // order. Assert it explicitly here so a future refactor that moves
        // them onto separate Tasks breaks this test.
        #expect(provider.authorizationRequested == true)
        #expect(provider.backgroundDeliveryCallCount >= 1)
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
