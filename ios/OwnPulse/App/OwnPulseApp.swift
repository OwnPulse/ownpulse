// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import BackgroundTasks
import SwiftUI
import UserNotifications

@main
struct OwnPulseApp: App {
    @State private var dependencies = AppDependencies()
    @Environment(\.scenePhase) private var scenePhase
    @UIApplicationDelegateAdaptor private var notificationDelegate: NotificationDelegate

    var body: some Scene {
        WindowGroup {
            rootView
                .environment(dependencies)
                .onOpenURL { url in
                    // Widget/deep-link routing first; fall through to the
                    // auth callback handler only for non-deep-link URLs.
                    if !dependencies.handleDeepLink(url) {
                        dependencies.authService.handleCallback(url: url)
                    }
                }
                .onAppear {
                    registerBackgroundTasks()
                    configureNotificationDelegate()
                    // Bootstrap the BGAppRefresh chain and live observer — this
                    // is where we break the chicken-and-egg in the old code.
                    // `scheduleNextSync()` was only called from inside the
                    // background task handler, so the chain never started.
                    dependencies.bootstrapAutoSync()
                }
                .task {
                    await dependencies.featureFlagService.fetch()
                }
        }
        .onChange(of: scenePhase) { _, newPhase in
            // Delegate to a pure method on AppDependencies so the policy is
            // unit-testable. See `AppDependenciesScenePhaseTests`.
            dependencies.handleScenePhase(newPhase)
        }
    }

    @ViewBuilder
    private var rootView: some View {
        #if DEBUG
        // XCUITest hook: render the widget views in isolation so the snapshot
        // UI test can assert all three families. Never reachable in release.
        if ProcessInfo.processInfo.arguments.contains("-WidgetSnapshotHarness") {
            WidgetSnapshotHarness()
        } else {
            ContentView()
        }
        #else
        ContentView()
        #endif
    }

    private func registerBackgroundTasks() {
        BGTaskScheduler.shared.register(
            forTaskWithIdentifier: "health.ownpulse.sync",
            using: nil
        ) { task in
            guard let refreshTask = task as? BGAppRefreshTask else { return }
            nonisolated(unsafe) let bgTask = refreshTask
            Task {
                await BackgroundTaskHandler.handleSync(
                    task: bgTask,
                    syncEngine: dependencies.syncEngine
                )
            }
        }
    }

    private func configureNotificationDelegate() {
        UNUserNotificationCenter.current().delegate = notificationDelegate

        notificationDelegate.onDeviceToken = { [dependencies] tokenData in
            Task { @MainActor in
                await dependencies.notificationManager.registerDeviceToken(tokenData)
            }
        }

        notificationDelegate.onNotificationTap = { _ in
            // Notification tap navigates to Dashboard (tab 0) — handled by
            // ContentView's default tab selection.
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
