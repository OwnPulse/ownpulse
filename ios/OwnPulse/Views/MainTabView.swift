// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct MainTabView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var selectedTab = 0
    @State private var showHealthKitPrompt = false

    var body: some View {
        TabView(selection: $selectedTab) {
            Tab("Dashboard", systemImage: "heart.text.clipboard", value: 0) {
                NavigationStack {
                    DashboardView()
                }
            }

            Tab("Protocols", systemImage: "list.bullet.clipboard", value: 1) {
                NavigationStack {
                    ProtocolsListView()
                }
            }

            Tab("Log", systemImage: "plus.circle", value: 2) {
                NavigationStack {
                    LogView()
                }
            }

            Tab("Explore", systemImage: "chart.xyaxis.line", value: 3) {
                NavigationStack {
                    ExploreView()
                }
            }

            Tab("Settings", systemImage: "gear", value: 4) {
                NavigationStack {
                    SettingsView()
                }
            }
        }
        .tint(OPColor.terracotta)
        .accessibilityIdentifier("mainTabView")
        .task {
            if !UserDefaults.standard.bool(forKey: "hasSeenHealthKitPrompt") {
                showHealthKitPrompt = true
            }
        }
        .sheet(isPresented: $showHealthKitPrompt) {
            HealthKitPromptSheet(
                onConnect: {
                    Task {
                        try? await dependencies.healthKitProvider.requestAuthorization()
                    }
                    UserDefaults.standard.set(true, forKey: "hasSeenHealthKitPrompt")
                    showHealthKitPrompt = false
                },
                onDismiss: {
                    UserDefaults.standard.set(true, forKey: "hasSeenHealthKitPrompt")
                    showHealthKitPrompt = false
                }
            )
        }
    }
}
