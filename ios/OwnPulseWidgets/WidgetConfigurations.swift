// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import WidgetKit

// These `Widget` configurations reference `OwnPulseProvider` (the extension's
// `TimelineProvider`) and therefore live ONLY in the widget extension target.
// The matching `*WidgetView` views are shared with the app target so the
// DEBUG snapshot harness can render them — but the providers and configs are
// never visible to the app.

/// Lock-screen widget showing whether today's subjective check-in is filled.
/// Tapping deep-links into the check-in form.
struct TodayCheckinWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: WidgetSharedConstants.todayCheckinKind, provider: OwnPulseProvider()) { entry in
            TodayCheckinWidgetView(entry: entry)
                .widgetURL(URL(string: "ownpulse://log?form=checkin"))
        }
        .configurationDisplayName("Today's Check-in")
        .description("See at a glance whether you've logged today's check-in.")
        .supportedFamilies([.accessoryCircular, .accessoryRectangular])
    }
}

/// Shows the latest hero metric (resting HR / HRV / sleep duration) on a
/// lock-screen rectangular family and a home-screen small family.
struct HeroMetricWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: WidgetSharedConstants.heroMetricKind, provider: OwnPulseProvider()) { entry in
            HeroMetricWidgetView(entry: entry)
                .widgetURL(URL(string: "ownpulse://log?form=checkin"))
        }
        .configurationDisplayName("Hero Metric")
        .description("Your latest headline health metric at a glance.")
        .supportedFamilies([.accessoryRectangular, .systemSmall])
    }
}

/// A one-tap lock-screen shortcut that deep-links into the Log screen with the
/// intervention form pre-selected — the most common "log something now" path.
struct QuickLogWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: WidgetSharedConstants.quickLogKind, provider: OwnPulseProvider()) { entry in
            QuickLogWidgetView(entry: entry)
                .widgetURL(URL(string: "ownpulse://log?form=intervention"))
        }
        .configurationDisplayName("Quick Log")
        .description("Jump straight to logging an intervention.")
        .supportedFamilies([.accessoryCircular])
    }
}
