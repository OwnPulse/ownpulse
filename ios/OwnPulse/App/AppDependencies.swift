// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit
import MetricKit
import Observation
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

    init(
        keychainService: KeychainServiceProtocol? = nil,
        networkClient: NetworkClientProtocol? = nil,
        healthKitProvider: HealthKitProviderProtocol? = nil
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

        self.syncScheduler = SyncScheduler()

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

        // Wire auto-sync hooks: first sync on login, subscribe to HealthKit
        // observer updates so new samples trigger a debounced sync.
        self.authService.onLoginSuccess = { [weak self] in
            guard let self else { return }
            syncLogger.info("Login succeeded — starting initial sync and HealthKit observer")
            Task { [syncEngine = self.syncEngine] in
                await syncEngine.sync()
            }
            Task { [coordinator = self.syncCoordinator] in
                await coordinator.start()
            }
        }
    }

    /// Called once on app launch. Kicks off the BGAppRefresh chain if the
    /// user is authenticated, and starts the HealthKit observer subscription
    /// so foreground HealthKit updates trigger automatic syncs.
    ///
    /// Safe to call multiple times — both `SyncScheduler.scheduleNextSync`
    /// and `SyncCoordinator.start` are idempotent.
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
    }
}
