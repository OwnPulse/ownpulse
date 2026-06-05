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
            rootView
                .environment(dependencies)
                .preferredColorScheme(
                    ColorSchemePreference.from(rawValue: colorSchemeRaw).colorScheme
                )
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
        // CRITICAL: `BGTaskScheduler` runs this launch handler on a BACKGROUND
        // dispatch queue, NOT the main actor. The `launchHandler` parameter is
        // not `@Sendable`, so a closure defined in this `@MainActor` method
        // that captures `@MainActor` state (`dependencies`) would be inferred
        // `@MainActor`-isolated, and the Swift 6 runtime would trap with an
        // executor-isolation assertion the first time a real background refresh
        // fired (`_swift_task_checkIsolatedSwift` / `dispatch_assert_queue`).
        //
        // Marking the closure `@Sendable` forces it non-isolated, so it is safe
        // to run off the main actor. `BackgroundTaskHandler` makes no synchronous
        // main-actor access; nothing in the synchronous body of the closure
        // asserts main-actor isolation.
        //
        // We resolve `syncEngine` HERE — `registerBackgroundTasks()` is itself
        // `@MainActor` (this is a SwiftUI `App`), so reading the `@MainActor`
        // `dependencies` is a synchronous, in-isolation access. `SyncEngine` is
        // an `actor` (`Sendable`), so capturing only it into the launch closure
        // avoids capturing the non-`Sendable`-region `dependencies` graph.
        let syncEngine = dependencies.syncEngine
        BGTaskScheduler.shared.register(
            forTaskWithIdentifier: "health.ownpulse.sync",
            using: nil
        ) { @Sendable [syncEngine] task in
            // `BGTask` is not `Sendable`. We hand it across the `Task` boundary
            // explicitly — a legitimate use of `nonisolated(unsafe)` (the system
            // delivers `task` exactly once), NOT a paper-over of the isolation
            // bug, which is fixed by the `@Sendable` closure above.
            //
            // The `Task` is deliberately non-isolated (no `@MainActor`): keeping
            // `bgTask` in the non-isolated region means it never crosses an actor
            // boundary, so it needs neither a `Sendable` conformance (impossible
            // for `BGTask`) nor a `sending` parameter (which would forbid the
            // tests from inspecting the task after the call). Under Release
            // whole-module optimization, sending `bgTask` into a `@MainActor`
            // `Task` and back out to the non-isolated handler is what the
            // compiler rejects ("sending value of non-Sendable type 'BGTask'").
            // The only capture is the `Sendable` `syncEngine`.
            nonisolated(unsafe) let bgTask = task
            Task {
                await BackgroundTaskHandler.handleSync(
                    task: bgTask,
                    syncEngine: syncEngine
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
