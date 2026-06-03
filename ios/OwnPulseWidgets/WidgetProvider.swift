// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import WidgetKit

/// Reads the snapshot the main app published into the app group and produces a
/// single-entry timeline. We rely on `WidgetCenter.reloadAllTimelines()` (fired
/// by the app after each sync/dashboard refresh) for freshness rather than a
/// polling cadence, but we also schedule a conservative refresh so the widget
/// doesn't go stale if the app isn't opened for a while.
struct OwnPulseProvider: TimelineProvider {
    /// Shared reader. The widget extension is a member of the same app group,
    /// so the production `WidgetDataPublisher` reads the app-group defaults.
    /// NOTE: the extension only ever calls `load()` here — it is a read-only
    /// consumer. `WidgetDataPublisher.publish()` / `reload()` exist for the
    /// app target's writer path and are intentionally unused from the widget.
    private let publisher = WidgetDataPublisher()

    func placeholder(in context: Context) -> OwnPulseEntry {
        OwnPulseEntry(date: Date(), snapshot: .placeholder)
    }

    func getSnapshot(in context: Context, completion: @escaping (OwnPulseEntry) -> Void) {
        completion(currentEntry())
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<OwnPulseEntry>) -> Void) {
        let now = Date()
        let entry = currentEntry(now: now)
        // Ask the system to refresh in ~1 hour as a backstop; the app's
        // explicit reloads handle the common case immediately. Derive the
        // backstop from the same `now` so a single Date drives the whole
        // timeline (no second clock read that could disagree).
        let next = Calendar.current.date(byAdding: .hour, value: 1, to: now) ?? now.addingTimeInterval(3600)
        completion(Timeline(entries: [entry], policy: .after(next)))
    }

    private func currentEntry(now: Date = Date()) -> OwnPulseEntry {
        OwnPulseEntry(date: now, snapshot: publisher.load() ?? .placeholder)
    }
}
