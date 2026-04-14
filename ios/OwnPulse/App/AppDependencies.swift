// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit
import MetricKit
import Observation

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
    let syncScheduler: SyncScheduler
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

        self.syncEngine = SyncEngine(
            networkClient: network,
            healthKitProvider: self.healthKitProvider,
            clinicalRecordProvider: self.clinicalRecordProvider,
            medicationSyncProvider: self.medicationSyncProvider,
            offlineQueue: offlineQueue,
            anchorStore: anchorStore
        )

        self.syncScheduler = SyncScheduler()

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
    }
}
