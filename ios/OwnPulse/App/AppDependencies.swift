// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit
import MetricKit
import Observation
import SwiftUI
import os

private let syncLogger = Logger(subsystem: "health.ownpulse.app", category: "sync")

@Observable
@MainActor
final class AppDependencies {
    let keychainService: KeychainServiceProtocol
    let networkClient: NetworkClientProtocol
    let authService: AuthService
    let healthKitProvider: HealthKitProviderProtocol
    let databaseManager: DatabaseManager
    let offlineQueue: OfflineQueueProtocol
    let anchorStore: AnchorStore
    let clinicalRecordProvider: ClinicalRecordProviderProtocol?
    let medicationSyncProvider: (any Sendable)?
    let syncEngine: SyncEngine
    let syncProgress: SyncProgress
    let syncScheduler: SyncScheduler
    let syncCoordinator: SyncCoordinator
    let adminService: AdminService
    let notificationManager: NotificationManager
    let featureFlagService: FeatureFlagService
    private var crashReporter: CrashReporter?

    /// Currently selected root tab. Lives here so cards on Dashboard can
    /// switch tabs (e.g. "Log Today's Check-in" jumps to the Log tab).
    var selectedTab: Int = 0

    init(
        keychainService: KeychainServiceProtocol? = nil,
        networkClient: NetworkClientProtocol? = nil,
        healthKitProvider: HealthKitProviderProtocol? = nil,
        syncScheduler: SyncScheduler? = nil
    ) {
        let keychain = keychainService ?? KeychainService()
        self.keychainService = keychain

        let network = networkClient ?? NetworkClient(keychainService: keychain)
        self.networkClient = network

        self.authService = AuthService(
            networkClient: network,
            keychainService: keychain
        )

        self.healthKitProvider = healthKitProvider ?? HealthKitProvider()

        self.clinicalRecordProvider = HKHealthStore.isHealthDataAvailable()
            ? ClinicalRecordProvider() : nil

        #if swift(>=6.3)
        if #available(iOS 26.0, *), HKHealthStore.isHealthDataAvailable() {
            self.medicationSyncProvider = MedicationSyncProvider()
        } else {
            self.medicationSyncProvider = nil
        }
        #else
        self.medicationSyncProvider = nil
        #endif

        self.databaseManager = DatabaseManager()

        self.offlineQueue = OfflineQueue(databaseManager: databaseManager)
        self.anchorStore = AnchorStore(databaseManager: databaseManager)

        self.syncProgress = SyncProgress()

        #if canImport(UIKit)
        let backgroundTaskHost: BackgroundTaskHost = UIKitBackgroundTaskHost()
        #else
        let backgroundTaskHost: BackgroundTaskHost? = nil
        #endif

        self.syncEngine = SyncEngine(
            networkClient: network,
            healthKitProvider: self.healthKitProvider,
            clinicalRecordProvider: self.clinicalRecordProvider,
            medicationSyncProvider: self.medicationSyncProvider,
            offlineQueue: offlineQueue,
            anchorStore: anchorStore,
            progress: self.syncProgress,
            backgroundTaskHost: backgroundTaskHost
        )

        self.syncScheduler = syncScheduler ?? SyncScheduler()

        self.syncCoordinator = SyncCoordinator(
            healthKitProvider: self.healthKitProvider,
            syncEngine: self.syncEngine
        )

        self.adminService = AdminService(networkClient: network)

        self.notificationManager = NotificationManager(networkClient: network)

        self.featureFlagService = FeatureFlagService(networkClient: network)

        // Telemetry — consent-gated crash reporting and flow tracking
        if TelemetrySettings.isEnabled {
            let reporter = CrashReporter(networkClient: network)
            MXMetricManager.shared.add(reporter)
            self.crashReporter = reporter
        }
        Task { await FlowTracker.shared.configure(networkClient: network) }

        // Wire auto-sync hooks.
        //
        // On login: run the full bootstrap (initial sync, observer start,
        // scheduler, background delivery). Previously the login handler only
        // kicked an initial sync; on a fresh install the user isn't authed
        // at `.onAppear` so bootstrapAutoSync() early-returns, and the
        // background paths never start until the next app relaunch.
        //
        // On logout: tear everything down BEFORE the keychain is cleared so
        // the coordinator can't fire a 401 on an expired token and iOS
        // isn't left waking us for a user that's signed out.
        self.authService.onLoginSuccess = { [weak self] in
            guard let self else { return }
            syncLogger.info("Login succeeded — running full auto-sync bootstrap")
            self.bootstrapAutoSync()
        }
        self.authService.onLogout = { [weak self] in
            guard let self else { return }
            syncLogger.info("Logout — tearing down HealthKit observer and background delivery")
            await self.teardownAutoSync()
        }
    }

    /// Called once on app launch (and on successful login). If the user is
    /// authenticated, schedules the next BGAppRefresh, subscribes the
    /// SyncCoordinator to HealthKit observer events, enables HealthKit
    /// background delivery, and fires an initial sync.
    ///
    /// Safe to call multiple times — each sub-operation is idempotent:
    /// - `SyncScheduler.scheduleNextSync` replaces the existing request
    /// - `SyncCoordinator.start` guards against double-subscription
    /// - `enableBackgroundDelivery` coalesces repeated registrations
    /// - `SyncEngine.sync` early-returns when a sync is already in flight
    func bootstrapAutoSync() {
        guard authService.isAuthenticated else { return }

        syncScheduler.scheduleNextSync()

        Task { [coordinator = syncCoordinator] in
            await coordinator.start()
        }

        // Enable background delivery so iOS wakes us when HealthKit data is
        // written outside of a foreground session. Best-effort; log and
        // continue if it fails (e.g. the user revoked HealthKit access).
        let hkProvider = healthKitProvider
        Task {
            do {
                try await hkProvider.enableBackgroundDelivery()
            } catch {
                syncLogger.error("enableBackgroundDelivery failed: \(error.localizedDescription, privacy: .public)")
            }
        }

        // Initial sync so the dashboard reflects today's data without the
        // user having to tap Sync Now. The engine's re-entrancy guard
        // swallows overlapping calls if the observer races ahead.
        Task { [syncEngine] in
            await syncEngine.sync()
        }
    }

    /// Stop all auto-sync work. Called from the logout hook BEFORE the
    /// keychain is cleared so cleanup can safely issue any final requests
    /// if needed.
    func teardownAutoSync() async {
        await syncCoordinator.stop()
        do {
            try await healthKitProvider.disableAllBackgroundDelivery()
        } catch {
            syncLogger.error("disableAllBackgroundDelivery failed: \(error.localizedDescription, privacy: .public)")
        }
    }

    /// Pure handler for `ScenePhase` changes. Returns `true` when a sync was
    /// triggered, `false` otherwise. Extracted from the SwiftUI scene body
    /// so the logic can be unit-tested without spinning up a Scene.
    ///
    /// Policy:
    /// - `.active` while authenticated → fire sync (the engine's re-entrancy
    ///   guard coalesces rapid scene-phase flips)
    /// - Unauthenticated or any non-active phase → no-op
    @discardableResult
    func handleScenePhase(_ phase: ScenePhase) -> Bool {
        guard phase == .active else { return false }
        guard authService.isAuthenticated else { return false }

        Task { [syncEngine] in
            await syncEngine.sync()
        }
        return true
    }
}
