// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import os
#if canImport(WidgetKit)
import WidgetKit
#endif

private let widgetLogger = Logger(subsystem: "health.ownpulse.app", category: "widgets")

/// Abstracts the app-group key/value store so ``WidgetDataPublisher`` can be
/// unit-tested without a real shared container (the test process isn't a
/// member of the app group).
protocol WidgetDefaultsStore: AnyObject, Sendable {
    func data(forKey key: String) -> Data?
    func set(_ data: Data?, forKey key: String)
}

// `WidgetDefaultsStore` requires `Sendable`. `UserDefaults` is thread-safe but
// not declared `Sendable` in the SDK, so we add an explicit retroactive
// `@unchecked Sendable` conformance (required for Swift 6 language mode).
extension UserDefaults: @retroactive @unchecked Sendable {}

extension UserDefaults: WidgetDefaultsStore {
    func set(_ data: Data?, forKey key: String) {
        set(data as Any?, forKey: key)
    }
}

/// Writes the latest widget-facing values into the shared app group and asks
/// WidgetKit to reload the timelines. The widgets are **read-only** consumers
/// of this data — they never write back.
///
/// This lives in the main app and is invoked on the sync/dashboard refresh
/// completion path, where the displayable values (today's check-in status and
/// the latest hero metric) are known.
final class WidgetDataPublisher: Sendable {
    private let store: WidgetDefaultsStore?
    /// Indirection over `WidgetCenter.reloadTimelines` so tests can observe
    /// reloads without WidgetKit side effects.
    private let reload: @Sendable () -> Void

    /// Production initializer — binds to the shared app group and the real
    /// WidgetKit reload. If the app group is misconfigured (no entitlement),
    /// `store` is `nil` and writes become no-ops rather than crashing.
    init(appGroupID: String = WidgetSharedConstants.appGroupID) {
        self.store = UserDefaults(suiteName: appGroupID)
        self.reload = {
            #if canImport(WidgetKit)
            WidgetCenter.shared.reloadAllTimelines()
            #endif
        }
        if self.store == nil {
            widgetLogger.error("Widget app group unavailable — snapshot publishing disabled")
        }
    }

    /// Test/seam initializer.
    init(store: WidgetDefaultsStore?, reload: @escaping @Sendable () -> Void) {
        self.store = store
        self.reload = reload
    }

    /// Persist a snapshot and reload widget timelines. No-op (logged) if the
    /// shared store is unavailable.
    func publish(_ snapshot: WidgetSnapshot) {
        guard let store else { return }
        do {
            let encoded = try JSONEncoder().encode(snapshot)
            store.set(encoded, forKey: WidgetSharedConstants.snapshotKey)
            reload()
        } catch {
            // Encoding a fixed-shape Codable struct should never fail; if it
            // somehow does, swallow it — widgets simply keep their last value.
            widgetLogger.error("Failed to encode widget snapshot: \(error.localizedDescription, privacy: .public)")
        }
    }

    /// Read back the current snapshot, or `nil` if none has been written yet.
    /// Used by the widget extension's timeline provider.
    func load() -> WidgetSnapshot? {
        guard let store, let data = store.data(forKey: WidgetSharedConstants.snapshotKey) else {
            return nil
        }
        return try? JSONDecoder().decode(WidgetSnapshot.self, from: data)
    }
}
