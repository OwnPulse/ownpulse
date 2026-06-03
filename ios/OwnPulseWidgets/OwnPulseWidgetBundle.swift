// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import WidgetKit

@main
struct OwnPulseWidgetBundle: WidgetBundle {
    var body: some Widget {
        TodayCheckinWidget()
        HeroMetricWidget()
        QuickLogWidget()
    }
}
