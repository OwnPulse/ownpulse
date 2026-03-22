// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct SettingsView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var showLogoutConfirmation = false
    @State private var hkAuthorized = false

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
        }
        .confirmationDialog("Sign out?", isPresented: $showLogoutConfirmation) {
            Button("Sign Out", role: .destructive) {
                Task {
                    await dependencies.authService.logout()
                }
            }
        }
    }
}
