// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import BackgroundTasks
import SwiftUI

@main
struct OwnPulseApp: App {
    @State private var dependencies = AppDependencies()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(dependencies)
                .onOpenURL { url in
                    dependencies.authService.handleCallback(url: url)
                }
                .onAppear {
                    registerBackgroundTasks()
                }
        }
    }

    private func registerBackgroundTasks() {
        BGTaskScheduler.shared.register(
            forTaskWithIdentifier: "health.ownpulse.sync",
            using: nil
        ) { task in
            guard let refreshTask = task as? BGAppRefreshTask else { return }
            Task {
                await BackgroundTaskHandler.handleSync(
                    task: refreshTask,
                    syncEngine: dependencies.syncEngine
                )
            }
        }
    }
}

struct ContentView: View {
    @Environment(AppDependencies.self) private var dependencies

    var body: some View {
        if dependencies.authService.isAuthenticated {
            MainTabView()
        } else {
            LoginView()
        }
    }
}
