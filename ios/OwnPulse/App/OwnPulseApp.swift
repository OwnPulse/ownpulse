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
    // Explicit light/dark/system appearance preference (mirrors the web
    // tri-state). @AppStorage persists it across relaunches.
    @AppStorage(ColorSchemePreference.storageKey) private var colorSchemeRaw =
        ColorSchemePreference.system.rawValue

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(dependencies)
                .preferredColorScheme(
                    ColorSchemePreference.from(rawValue: colorSchemeRaw).colorScheme
                )
                .onOpenURL { url in
                    dependencies.authService.handleCallback(url: url)
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
