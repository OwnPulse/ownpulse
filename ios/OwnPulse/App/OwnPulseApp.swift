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
            // `BGTask` is not `Sendable` and cannot be made so (non-final
            // imported ObjC class). The system delivers `task` exactly once, so
            // it is safe to carry into the work `Task` — but the compiler can't
            // know that. We wrap it in `UncheckedSendableBox` so the work `Task`
            // captures a `Sendable` value; without it the closure trips both
            // "sending value of non-Sendable type 'BGTask'" (Release WMO) and
            // "passing closure as a 'sending' parameter" (Debug). The box is the
            // explicit, narrow opt-out — `nonisolated(unsafe)` does not cover a
            // capture by a `sending` `Task` closure.
            //
            // The work `Task` is deliberately NON-isolated (no `@MainActor`):
            // that keeps `BackgroundTaskHandler.handleSync` off the main actor —
            // it makes no synchronous main-actor access and its expiration
            // handler fires on a background queue. The only other capture is the
            // `Sendable` `syncEngine`, resolved above on the main actor.
            let box = UncheckedSendableBox(task)
            Task {
                await BackgroundTaskHandler.handleSync(
                    task: box.value,
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

/// Carries a non-`Sendable` value across a concurrency boundary when the caller
/// can guarantee single-threaded hand-off the compiler can't see. Used to pass
/// a `BGTask` (a non-final imported ObjC class that cannot conform to
/// `Sendable`) into the non-isolated work `Task` of the background-refresh
/// launch handler, where the system delivers the task exactly once.
private struct UncheckedSendableBox<Value>: @unchecked Sendable {
    let value: Value
    init(_ value: Value) { self.value = value }
}
