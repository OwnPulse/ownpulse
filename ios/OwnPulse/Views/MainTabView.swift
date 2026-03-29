// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct MainTabView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var selectedTab = 0

    var body: some View {
        TabView(selection: $selectedTab) {
            Tab("Dashboard", systemImage: "heart.text.clipboard", value: 0) {
                NavigationStack {
                    DashboardView()
                }
            }

            Tab("Log", systemImage: "plus.circle", value: 1) {
                NavigationStack {
                    LogView()
                }
            }

            Tab("Explore", systemImage: "chart.xyaxis.line", value: 2) {
                NavigationStack {
                    ExploreWebView()
                }
            }

            Tab("Settings", systemImage: "gear", value: 3) {
                NavigationStack {
                    SettingsView()
                }
            }
        }
        .tint(OPColor.terracotta)
        .accessibilityIdentifier("mainTabView")
    }
}
