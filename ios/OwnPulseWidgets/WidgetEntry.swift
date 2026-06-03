// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import WidgetKit

/// Timeline entry carrying the latest ``WidgetSnapshot`` read from the shared
/// app group. All three widgets share this entry type — they differ only in
/// their views. Shared with the app target so the DEBUG widget harness (and
/// its XCUITest) can render the same views.
struct OwnPulseEntry: TimelineEntry {
    let date: Date
    let snapshot: WidgetSnapshot
}
