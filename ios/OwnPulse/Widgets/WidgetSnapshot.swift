// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

/// The single source of truth the lock-screen widgets read from. Written by
/// the main app via ``WidgetDataPublisher`` into the shared app group's
/// `UserDefaults`; read (never written) by the `OwnPulseWidgets` extension.
///
/// PRIVACY — read before adding fields:
/// This payload is NOT safe for sensitive data. The app-group `UserDefaults`
/// plist is `NSFileProtectionCompleteUntilFirstUnlock` at best, and a
/// lock-screen widget deliberately renders its contents on the LOCKED screen
/// — i.e. visible to anyone holding the device, without unlocking. So whatever
/// lands here is effectively at-rest-unprotected and shoulder-surfable.
///
/// Keep this minimal: a check-in boolean plus exactly ONE coarse vital
/// (resting HR) is the defensible maximum. Do NOT add symptom severities,
/// substance/intervention names, observation contents, notes, or any free
/// text — those are sensitive and must never surface on the lock screen
/// without a real, explicit per-widget opt-in. When in doubt, leave it out.
struct WidgetSnapshot: Codable, Sendable, Equatable {
    /// Whether today's subjective check-in has been filled in.
    var checkinFilledToday: Bool

    /// Display name of the current hero metric (e.g. "Resting Heart Rate").
    var heroMetricName: String

    /// Pre-formatted current value of the hero metric (e.g. "56").
    var heroMetricValue: String

    /// Unit string for the hero metric (e.g. "bpm").
    var heroMetricUnit: String

    /// Pre-formatted trend label, may be empty.
    var heroTrendText: String

    /// `true` when the trend is in the "good" direction (drives tint).
    var heroTrendIsPositive: Bool

    /// When the snapshot was last written by the app.
    var lastUpdated: Date

    /// Beyond this age the hero value is no longer trustworthy as "current" —
    /// the widget falls back to a placeholder dash rather than presenting a
    /// multi-day-old vital (and a stale trend) as if it were live. 24h matches
    /// the daily cadence of the metrics we surface.
    static let stalenessThreshold: TimeInterval = 24 * 60 * 60

    /// `true` when `lastUpdated` is older than ``stalenessThreshold`` relative
    /// to `now`, or when it's the epoch sentinel used by the placeholder.
    func isStale(asOf now: Date = Date()) -> Bool {
        now.timeIntervalSince(lastUpdated) > Self.stalenessThreshold
    }

    /// A neutral placeholder used for previews and the "no data yet" state.
    static let placeholder = WidgetSnapshot(
        checkinFilledToday: false,
        heroMetricName: "Resting Heart Rate",
        heroMetricValue: "—",
        heroMetricUnit: "bpm",
        heroTrendText: "",
        heroTrendIsPositive: true,
        lastUpdated: Date(timeIntervalSince1970: 0)
    )
}

/// Shared constants for the app-group data channel. Used by both the main app
/// (writer) and the widget extension (reader).
enum WidgetSharedConstants {
    /// App group both targets are members of. Read-only data sharing.
    static let appGroupID = "group.health.ownpulse.shared"

    /// Key under which the encoded ``WidgetSnapshot`` is stored.
    static let snapshotKey = "widgetSnapshot"

    /// WidgetKit kind identifiers — referenced by reloads and the bundle.
    static let todayCheckinKind = "TodayCheckinWidget"
    static let heroMetricKind = "HeroMetricWidget"
    static let quickLogKind = "QuickLogWidget"
}
