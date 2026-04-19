// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
#if canImport(UIKit)
import UIKit
#endif

/// Shared constant for "no background task active". Mirrors the semantics
/// of `UIBackgroundTaskIdentifier.invalid.rawValue` but avoids requiring
/// UIKit in the test target.
let invalidBackgroundTask: Int = 0

/// Abstraction over `UIApplication.begin/endBackgroundTask(_:)` so the sync
/// engine can request extra execution time without a direct dependency on
/// UIKit in test builds.
///
/// The real implementation (`UIKitBackgroundTaskHost`) calls straight through
/// to `UIApplication.shared`. Tests pass a mock that just records calls.
protocol BackgroundTaskHost: Sendable {
    /// Request continued execution while the app is backgrounded. Returns a
    /// token that must be paired with a later `end(_:)` call. If the system
    /// runs out of time before `end` is called, `expirationHandler` fires on
    /// the main thread — callers should end the task in that handler to
    /// avoid the system terminating the app.
    @MainActor func beginBackgroundTask(
        name: String,
        expirationHandler: @escaping @Sendable () -> Void
    ) -> Int

    /// End the background task identified by `id`. Safe to call multiple
    /// times with the same id — the underlying UIApplication call is a no-op
    /// for invalid identifiers, but callers should still guard against
    /// double-end to avoid logging false expirations.
    @MainActor func endBackgroundTask(_ id: Int)
}

#if canImport(UIKit)
/// Production implementation backed by `UIApplication.shared`.
struct UIKitBackgroundTaskHost: BackgroundTaskHost {
    @MainActor
    func beginBackgroundTask(
        name: String,
        expirationHandler: @escaping @Sendable () -> Void
    ) -> Int {
        let id = UIApplication.shared.beginBackgroundTask(
            withName: name,
            expirationHandler: expirationHandler
        )
        return id.rawValue
    }

    @MainActor
    func endBackgroundTask(_ id: Int) {
        guard id != UIBackgroundTaskIdentifier.invalid.rawValue else { return }
        UIApplication.shared.endBackgroundTask(UIBackgroundTaskIdentifier(rawValue: id))
    }
}
#endif
