// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct SleepEntryForm: View {
    @Bindable var viewModel: LogViewModel

    var body: some View {
        VStack(spacing: 16) {
            // Duration — hours + minutes
            HStack(spacing: 12) {
                TextField("Hours", text: $viewModel.sleepHours)
                    .textFieldStyle(.roundedBorder)
                    .keyboardType(.numberPad)
                    .accessibilityIdentifier("sleepHoursField")

                TextField("Minutes", text: $viewModel.sleepMinutes)
                    .textFieldStyle(.roundedBorder)
                    .keyboardType(.numberPad)
                    .accessibilityIdentifier("sleepMinutesField")
            }

            if viewModel.sleepMinutesOutOfRange {
                Text("Minutes must be 0–59. Use the hours field for 60 or more.")
                    .font(.caption)
                    .foregroundStyle(.red)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .accessibilityIdentifier("sleepMinutesHint")
            }

            // Date/Time
            DatePicker(
                "Slept on",
                selection: $viewModel.sleepDate,
                displayedComponents: [.date, .hourAndMinute]
            )
            .datePickerStyle(.compact)
            .accessibilityIdentifier("sleepDatePicker")

            // Submit
            Button {
                Task { await viewModel.submitSleep() }
            } label: {
                Group {
                    if viewModel.submitState == .submitting {
                        ProgressView()
                            .tint(.white)
                    } else {
                        Text("Log Sleep")
                            .fontWeight(.semibold)
                    }
                }
                .frame(maxWidth: .infinity)
                .frame(height: 50)
                .background(OPColor.teal)
                .foregroundStyle(.white)
                .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            .disabled(!viewModel.sleepIsValid || viewModel.submitState == .submitting)
            .sensoryFeedback(.success, trigger: viewModel.submitState == .success("Sleep saved"))
            .accessibilityIdentifier("saveSleepButton")
        }
    }
}
