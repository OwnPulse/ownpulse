// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct MainTabView: View {
    @Environment(AppDependencies.self) private var dependencies

    var body: some View {
        @Bindable var deps = dependencies

        TabView(selection: $deps.selectedTab) {
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
    }
}
