// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import HealthKit
import Testing
@testable import OwnPulse

@Suite("SettingsViewModel", .serialized)
@MainActor
struct SettingsViewModelTests {
    private func makeMethods() -> [AuthMethod] {
        [
            AuthMethod(
                id: "1",
                provider: "apple",
                email: "user@icloud.com",
                createdAt: Date()
            ),
            AuthMethod(
                id: "2",
                provider: "password",
                email: nil,
                createdAt: Date()
            ),
        ]
    }

    @Test("loadAuthMethods success sets isLoadingMethods and populates authMethods")
    func loadAuthMethodsSuccess() async {
        let mock = MockNetworkClient()
        let methods = makeMethods()
        mock.requestHandler = { _, _, _ in methods }

        let vm = SettingsViewModel(networkClient: mock)

        #expect(vm.isLoadingMethods == false)
        #expect(vm.authMethods.isEmpty)

        await vm.loadAuthMethods()

        #expect(vm.isLoadingMethods == false)
        #expect(vm.authMethods.count == 2)
        #expect(vm.authMethods[0].provider == "apple")
        #expect(vm.linkError == nil)
    }

    @Test("loadAuthMethods failure sets linkError")
    func loadAuthMethodsFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "internal")
        }

        let vm = SettingsViewModel(networkClient: mock)

        await vm.loadAuthMethods()

        #expect(vm.linkError == "Failed to load linked accounts")
        #expect(vm.isLoadingMethods == false)
    }

    @Test("unlinkMethod clears linkError and linkInfo then reloads")
    func unlinkMethodClearsState() async {
        let mock = MockNetworkClient()
        let methods = makeMethods()
        mock.requestHandler = { method, _, _ in
            // Both DELETE (unlink) and GET (reload) return [AuthMethod]
            return methods
        }

        let vm = SettingsViewModel(networkClient: mock)
        vm.linkError = "previous error"
        vm.linkInfo = "previous info"

        await vm.unlinkMethod("apple")

        #expect(vm.linkError == nil)
        #expect(vm.linkInfo == nil)
        // Verify it made a DELETE call and then reloaded (GET)
        #expect(mock.requestCalls.count == 2)
        #expect(mock.requestCalls[0].method == "DELETE")
        #expect(mock.requestCalls[1].method == "GET")
    }

    @Test("unlinkMethod error sets linkError")
    func unlinkMethodError() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "internal")
        }

        let vm = SettingsViewModel(networkClient: mock)

        await vm.unlinkMethod("apple")

        #expect(vm.linkError != nil)
        #expect(vm.linkError!.contains("Failed to unlink"))
    }

    @Test("unlinkMethod rejects invalid provider")
    func unlinkMethodInvalidProvider() async {
        let mock = MockNetworkClient()
        let vm = SettingsViewModel(networkClient: mock)

        await vm.unlinkMethod("../../admin")

        #expect(vm.linkError == "Invalid provider: ../../admin")
        #expect(mock.requestCalls.isEmpty)
    }

    @Test("linkAppleWithToken posts to /auth/link with correct body and reloads methods")
    func linkApplePostsAndReloads() async throws {
        let mock = MockNetworkClient()
        let methods = makeMethods()

        var capturedBody: LinkAuthRequest?
        mock.requestHandler = { method, path, body in
            if method == "POST" && path == Endpoints.authLink {
                if let req = body as? LinkAuthRequest {
                    capturedBody = req
                }
            }
            return methods
        }

        let vm = SettingsViewModel(networkClient: mock)

        try await vm.linkAppleWithToken("test-token")

        #expect(capturedBody?.provider == "apple")
        #expect(capturedBody?.idToken == "test-token")
        #expect(capturedBody?.password == nil)
        // POST + GET (reload)
        #expect(mock.requestCalls.count == 2)
        #expect(mock.requestCalls[0].method == "POST")
        #expect(mock.requestCalls[0].path == Endpoints.authLink)
        #expect(mock.requestCalls[1].method == "GET")
    }

    @Test("linkGoogle sets linkInfo not linkError")
    func linkGoogleSetsInfo() {
        let mock = MockNetworkClient()
        let vm = SettingsViewModel(networkClient: mock)

        vm.linkGoogle()

        #expect(vm.linkInfo != nil)
        #expect(vm.linkInfo!.contains("web dashboard"))
        #expect(vm.linkError == nil)
    }

    // MARK: - Notification tests

    @Test("loadNotificationStatus shows Enabled when authorized")
    func loadNotificationStatusAuthorized() async {
        let mock = MockNetworkClient()
        let notifMock = MockNotificationManager()
        notifMock.currentStatus = .authorized
        let vm = SettingsViewModel(networkClient: mock, notificationManager: notifMock)

        await vm.loadNotificationStatus()

        #expect(vm.notificationsEnabled == true)
        #expect(vm.notificationStatusText == "Enabled")
    }

    @Test("loadNotificationStatus shows Not Set Up when not determined")
    func loadNotificationStatusNotDetermined() async {
        let mock = MockNetworkClient()
        let notifMock = MockNotificationManager()
        notifMock.currentStatus = .notDetermined
        let vm = SettingsViewModel(networkClient: mock, notificationManager: notifMock)

        await vm.loadNotificationStatus()

        #expect(vm.notificationsEnabled == false)
        #expect(vm.notificationStatusText == "Not Set Up")
    }

    @Test("loadNotificationStatus shows Denied when denied")
    func loadNotificationStatusDenied() async {
        let mock = MockNetworkClient()
        let notifMock = MockNotificationManager()
        notifMock.currentStatus = .denied
        let vm = SettingsViewModel(networkClient: mock, notificationManager: notifMock)

        await vm.loadNotificationStatus()

        #expect(vm.notificationsEnabled == false)
        #expect(vm.notificationStatusText == "Denied")
    }

    @Test("toggleNotifications requests permission and updates state on grant")
    func toggleNotificationsGranted() async {
        let mock = MockNetworkClient()
        let notifMock = MockNotificationManager()
        notifMock.permissionGranted = true
        let vm = SettingsViewModel(networkClient: mock, notificationManager: notifMock)
        vm.notificationsEnabled = false

        await vm.toggleNotifications()

        #expect(notifMock.requestPermissionCallCount == 1)
        #expect(vm.notificationsEnabled == true)
        #expect(vm.notificationStatusText == "Enabled")
        #expect(vm.notificationError == nil)
    }

    @Test("toggleNotifications sets error when permission denied")
    func toggleNotificationsDenied() async {
        let mock = MockNetworkClient()
        let notifMock = MockNotificationManager()
        notifMock.permissionGranted = false
        let vm = SettingsViewModel(networkClient: mock, notificationManager: notifMock)
        vm.notificationsEnabled = false

        await vm.toggleNotifications()

        #expect(notifMock.requestPermissionCallCount == 1)
        #expect(vm.notificationsEnabled == false)
        #expect(vm.notificationStatusText == "Denied")
        #expect(vm.notificationError != nil)
        #expect(vm.notificationError!.contains("Settings"))
    }

    @Test("toggleNotifications does not request permission when already enabled")
    func toggleNotificationsAlreadyEnabled() async {
        let mock = MockNetworkClient()
        let notifMock = MockNotificationManager()
        let vm = SettingsViewModel(networkClient: mock, notificationManager: notifMock)
        vm.notificationsEnabled = true

        await vm.toggleNotifications()

        #expect(notifMock.requestPermissionCallCount == 0)
    }

    // MARK: - Medication connect

    #if swift(>=6.3)
    @Test("connectMedications surfaces no error when the user cancels the permission sheet")
    func connectMedicationsUserCancel() async throws {
        guard #available(iOS 26.0, *) else { return }
        let provider = StubMedicationSyncProvider()
        provider.authorizationError = HKError(.errorUserCanceled)
        let vm = SettingsViewModel(
            networkClient: MockNetworkClient(),
            notificationManager: MockNotificationManager(),
            medicationSyncProvider: provider
        )

        await vm.connectMedications()

        #expect(vm.medicationConnectError == nil)
        #expect(vm.medicationCount == 0)
    }

    @Test("connectMedications surfaces an error message on a real failure")
    func connectMedicationsFailure() async throws {
        guard #available(iOS 26.0, *) else { return }
        let provider = StubMedicationSyncProvider()
        provider.authorizationError = HKError(.errorHealthDataUnavailable)
        let vm = SettingsViewModel(
            networkClient: MockNetworkClient(),
            notificationManager: MockNotificationManager(),
            medicationSyncProvider: provider
        )

        await vm.connectMedications()

        #expect(vm.medicationConnectError != nil)
    }

    @Test("connectMedications success clears the error and refreshes the count")
    func connectMedicationsSuccess() async throws {
        guard #available(iOS 26.0, *) else { return }
        let provider = StubMedicationSyncProvider()
        provider.medicationCount = 3
        let vm = SettingsViewModel(
            networkClient: MockNetworkClient(),
            notificationManager: MockNotificationManager(),
            medicationSyncProvider: provider
        )
        vm.medicationConnectError = "stale"

        await vm.connectMedications()

        #expect(vm.medicationConnectError == nil)
        #expect(vm.medicationCount == 3)
    }
    #endif
}

#if swift(>=6.3)
@available(iOS 26.0, *)
private final class StubMedicationSyncProvider: MedicationSyncProviderProtocol, @unchecked Sendable {
    var authorizationError: Error?
    var medicationCount = 0

    func requestAuthorization() async throws {
        if let authorizationError { throw authorizationError }
    }

    func authorizedMedicationCount() async throws -> Int { medicationCount }

    func queryDoseEvents(anchor: Data?) async throws -> (records: [MedicationDoseRecord], newAnchor: Data?) {
        ([], nil)
    }
}
#endif
