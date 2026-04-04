// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import AuthenticationServices
import SwiftUI
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "settings")

@Observable
@MainActor
final class SettingsViewModel {
    var authMethods: [AuthMethod] = []
    var isLoadingMethods = false
    var linkError: String?
    var linkInfo: String?
    var notificationsEnabled = false
    var notificationStatusText = "Unknown"
    var notificationError: String?

    private let networkClient: NetworkClientProtocol
    private let notificationManager: NotificationManagerProtocol

    init(
        networkClient: NetworkClientProtocol,
        notificationManager: NotificationManagerProtocol? = nil
    ) {
        self.networkClient = networkClient
        self.notificationManager = notificationManager ?? NotificationManager(networkClient: networkClient)
    }

    func loadAuthMethods() async {
        isLoadingMethods = true
        do {
            authMethods = try await networkClient.request(
                method: "GET",
                path: Endpoints.authMethods,
                body: Optional<String>.none
            )
        } catch {
            logger.error("Failed to load auth methods: \(error.localizedDescription, privacy: .public)")
            linkError = "Failed to load linked accounts"
        }
        isLoadingMethods = false
    }

    private static let allowedProviders: Set<String> = ["apple", "google", "local"]

    func unlinkMethod(_ provider: String) async {
        linkError = nil
        linkInfo = nil

        guard Self.allowedProviders.contains(provider) else {
            linkError = "Invalid provider: \(provider)"
            return
        }

        do {
            let _: [AuthMethod] = try await networkClient.request(
                method: "DELETE",
                path: "\(Endpoints.authLink)/\(provider)",
                body: Optional<String>.none
            )
            await loadAuthMethods()
        } catch {
            logger.error("Failed to unlink \(provider, privacy: .public): \(error.localizedDescription, privacy: .public)")
            linkError = "Failed to unlink \(provider.capitalized): \(error.localizedDescription)"
        }
    }

    func linkApple() async {
        linkError = nil
        linkInfo = nil
        do {
            let credential = try await AppleAuthHelper.performAppleAuth()

            guard let idTokenData = credential.identityToken,
                  let idToken = String(data: idTokenData, encoding: .utf8) else {
                throw AuthError.invalidCallback
            }

            try await linkAppleWithToken(idToken)
        } catch {
            logger.error("Failed to link Apple: \(error.localizedDescription, privacy: .public)")
            linkError = "Failed to link Apple account: \(error.localizedDescription)"
        }
    }

    /// Testable portion of Apple account linking: posts the identity token to
    /// the backend and reloads auth methods.
    func linkAppleWithToken(_ idToken: String) async throws {
        let body = LinkAuthRequest(provider: "apple", idToken: idToken, password: nil)
        let _: [AuthMethod] = try await networkClient.request(
            method: "POST",
            path: Endpoints.authLink,
            body: body
        )
        await loadAuthMethods()
    }

    func linkGoogle() {
        linkError = nil
        linkInfo = "To link a Google account, use the web dashboard."
    }

    func loadNotificationStatus() async {
        let status = await notificationManager.authorizationStatus()
        switch status {
        case .authorized, .provisional, .ephemeral:
            notificationsEnabled = true
            notificationStatusText = "Enabled"
        case .denied:
            notificationsEnabled = false
            notificationStatusText = "Denied"
        case .notDetermined:
            notificationsEnabled = false
            notificationStatusText = "Not Set Up"
        @unknown default:
            notificationsEnabled = false
            notificationStatusText = "Unknown"
        }
    }

    func toggleNotifications() async {
        notificationError = nil
        if !notificationsEnabled {
            let granted = await notificationManager.requestPermission()
            if granted {
                notificationsEnabled = true
                notificationStatusText = "Enabled"
            } else {
                notificationError = "Permission denied. Enable notifications in Settings."
                notificationStatusText = "Denied"
            }
        }
    }
}

// MARK: - SettingsView

struct SettingsView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var showLogoutConfirmation = false
    @State private var showUnlinkConfirmation = false
    @State private var unlinkProvider: String?
    @State private var hkAuthorized = false
    @State private var clinicalRecordsSyncEnabled = ClinicalRecordSettings.isSyncEnabled
    @State private var viewModel: SettingsViewModel?

    private var isAdmin: Bool {
        guard let tokenData = try? dependencies.keychainService.load(
            key: AuthService.accessTokenKey
        ),
            let token = String(data: tokenData, encoding: .utf8),
            let payload = JWTDecoder.decode(token)
        else {
            return false
        }
        return payload.role == "admin"
    }

    var body: some View {
        List {
            Section("HealthKit") {
                HStack {
                    Text("Authorization")
                    Spacer()
                    Text(hkAuthorized ? "Granted" : "Not Authorized")
                        .foregroundStyle(.secondary)
                }
                .accessibilityIdentifier("hkAuthStatus")

                if !hkAuthorized {
                    Button("Request Access") {
                        Task {
                            try? await dependencies.healthKitProvider.requestAuthorization()
                            hkAuthorized = dependencies.healthKitProvider.isAuthorized()
                        }
                    }
                    .accessibilityIdentifier("requestHKAccessButton")
                }
            }

            Section("Health Records") {
                Toggle("Sync Lab Results", isOn: $clinicalRecordsSyncEnabled)
                    .onChange(of: clinicalRecordsSyncEnabled) { _, newValue in
                        ClinicalRecordSettings.isSyncEnabled = newValue
                        if newValue {
                            Task {
                                try? await dependencies.clinicalRecordProvider?.requestAuthorization()
                            }
                        }
                    }
                Text("Import lab results from Epic, MyChart, Quest Diagnostics, and other connected health providers.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            if let vm = viewModel {
                notificationsSection(vm: vm)
                linkedAccountsSection(vm: vm)
            }

            if isAdmin {
                Section("Administration") {
                    NavigationLink("User Management") {
                        AdminView()
                    }
                    .accessibilityIdentifier("userManagementLink")
                }
            }

            Section("Dashboard") {
                Link("Open Web Dashboard", destination: AppConfig.webDashboardURL)
                    .accessibilityIdentifier("openDashboardLink")
            }

            Section("About") {
                HStack {
                    Text("Version")
                    Spacer()
                    Text(AppConfig.versionString)
                        .foregroundStyle(.secondary)
                }
                .accessibilityIdentifier("appVersion")
            }

            Section {
                Button("Sign Out", role: .destructive) {
                    showLogoutConfirmation = true
                }
                .accessibilityIdentifier("logoutButton")
            }
        }
        .navigationTitle("Settings")
        .onAppear {
            hkAuthorized = dependencies.healthKitProvider.isAuthorized()
            if viewModel == nil {
                viewModel = SettingsViewModel(
                    networkClient: dependencies.networkClient,
                    notificationManager: dependencies.notificationManager
                )
            }
            Task {
                await viewModel?.loadAuthMethods()
                await viewModel?.loadNotificationStatus()
            }
        }
        .confirmationDialog("Sign out?", isPresented: $showLogoutConfirmation) {
            Button("Sign Out", role: .destructive) {
                Task {
                    await dependencies.authService.logout()
                }
            }
        }
        .confirmationDialog(
            "Unlink \(unlinkProvider?.capitalized ?? "") account?",
            isPresented: $showUnlinkConfirmation
        ) {
            Button("Unlink", role: .destructive) {
                if let provider = unlinkProvider {
                    Task { await viewModel?.unlinkMethod(provider) }
                }
            }
        }
    }

    @ViewBuilder
    private func notificationsSection(vm: SettingsViewModel) -> some View {
        Section("Notifications") {
            HStack {
                Text("Dose Reminders")
                Spacer()
                Text(vm.notificationStatusText)
                    .foregroundStyle(.secondary)
            }
            .accessibilityIdentifier("notificationStatus")

            if !vm.notificationsEnabled {
                Button("Enable Notifications") {
                    Task { await vm.toggleNotifications() }
                }
                .accessibilityIdentifier("enableNotificationsButton")
            }

            if let error = vm.notificationError {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .accessibilityIdentifier("notificationError")
            }

            Text("Receive reminders when protocol doses are due. Configure notification times per protocol run.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    @ViewBuilder
    private func linkedAccountsSection(vm: SettingsViewModel) -> some View {
        Section("Linked Accounts") {
            if vm.isLoadingMethods {
                ProgressView()
                    .accessibilityIdentifier("linkedAccountsLoading")
            } else {
                ForEach(vm.authMethods) { method in
                    HStack {
                        Image(systemName: iconForProvider(method.provider))
                            .frame(width: 24)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(method.provider.capitalized)
                            if let email = method.email {
                                Text(email)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        Spacer()
                        if vm.authMethods.count > 1 {
                            Button("Unlink", role: .destructive) {
                                unlinkProvider = method.provider
                                showUnlinkConfirmation = true
                            }
                            .accessibilityIdentifier("unlink-\(method.provider)")
                        }
                    }
                }

                if !vm.authMethods.contains(where: { $0.provider == "apple" }) {
                    Button("Link Apple Account") {
                        Task { await vm.linkApple() }
                    }
                    .accessibilityIdentifier("linkAppleButton")
                }

                if !vm.authMethods.contains(where: { $0.provider == "google" }) {
                    Button("Link Google Account") {
                        vm.linkGoogle()
                    }
                    .accessibilityIdentifier("linkGoogleButton")
                }

                if let info = vm.linkInfo {
                    Text(info)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .accessibilityIdentifier("linkInfo")
                }

                if let error = vm.linkError {
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.red)
                        .accessibilityIdentifier("linkError")
                }
            }
        }
    }

    private func iconForProvider(_ provider: String) -> String {
        switch provider.lowercased() {
        case "apple":
            return "applelogo"
        case "google":
            return "globe"
        case "local":
            return "key.fill"
        default:
            return "person.circle"
        }
    }
}
