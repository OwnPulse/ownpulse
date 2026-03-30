// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit
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
    let syncEngine: SyncEngine
    let syncScheduler: SyncScheduler
    let adminService: AdminService

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

        self.databaseManager = DatabaseManager()

        self.offlineQueue = OfflineQueue(databaseManager: databaseManager)
        self.anchorStore = AnchorStore(databaseManager: databaseManager)

        self.syncEngine = SyncEngine(
            networkClient: network,
            healthKitProvider: self.healthKitProvider,
            clinicalRecordProvider: self.clinicalRecordProvider,
            offlineQueue: offlineQueue,
            anchorStore: anchorStore
        )

        self.syncScheduler = SyncScheduler()

        self.adminService = AdminService(networkClient: network)
    }
}
