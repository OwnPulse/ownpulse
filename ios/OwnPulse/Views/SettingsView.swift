// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct SettingsView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var showLogoutConfirmation = false
    @State private var hkAuthorized = false

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
