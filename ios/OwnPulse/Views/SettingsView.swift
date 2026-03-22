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

    private let networkClient: NetworkClientProtocol

    init(networkClient: NetworkClientProtocol) {
        self.networkClient = networkClient
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

            let body = LinkAuthRequest(provider: "apple", idToken: idToken, password: nil)
            let _: [AuthMethod] = try await networkClient.request(
                method: "POST",
                path: Endpoints.authLink,
                body: body
            )
            await loadAuthMethods()
        } catch {
            logger.error("Failed to link Apple: \(error.localizedDescription, privacy: .public)")
            linkError = "Failed to link Apple account: \(error.localizedDescription)"
        }
    }

    func linkGoogle() {
        linkError = nil
        linkInfo = "To link a Google account, use the web dashboard."
    }
}

// MARK: - SettingsView

struct SettingsView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var showLogoutConfirmation = false
    @State private var showUnlinkConfirmation = false
    @State private var unlinkProvider: String?
    @State private var hkAuthorized = false
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

            if let vm = viewModel {
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
                    networkClient: dependencies.networkClient
                )
            }
            Task { await viewModel?.loadAuthMethods() }
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
        case "password":
            return "key.fill"
        default:
            return "person.circle"
        }
    }
}
