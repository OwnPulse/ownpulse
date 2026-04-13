// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct DateRangeSegment: View {
    @Binding var selection: DateRangePreset

    var body: some View {
        Picker("Range", selection: $selection) {
            ForEach(DateRangePreset.allCases, id: \.self) { preset in
                Text(preset.rawValue).tag(preset)
            }
        }
        .pickerStyle(.segmented)
        .accessibilityIdentifier("dateRangeSegment")
    }
}
