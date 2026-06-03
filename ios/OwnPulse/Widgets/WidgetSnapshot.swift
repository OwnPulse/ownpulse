// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

/// The single source of truth the lock-screen widgets read from. Written by
/// the main app via ``WidgetDataPublisher`` into the shared app group's
/// `UserDefaults`; read (never written) by the `OwnPulseWidgets` extension.
///
/// Kept deliberately tiny: one latest value per surface, no history. The app
/// group container is on-device only and protected by iOS data protection, so
/// the latest hero value living here is acceptable — but we still store the
/// minimum needed to render and nothing resembling a record stream.
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
