// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct ExerciseEntryForm: View {
    @Bindable var viewModel: LogViewModel

    var body: some View {
        VStack(spacing: 16) {
            // Duration in minutes
            TextField("Duration (minutes)", text: $viewModel.exerciseMinutes)
                .textFieldStyle(.roundedBorder)
                .keyboardType(.numberPad)
                .accessibilityIdentifier("exerciseMinutesField")

            // Date/Time
            DatePicker(
                "Performed at",
                selection: $viewModel.exerciseDate,
                displayedComponents: [.date, .hourAndMinute]
            )
            .datePickerStyle(.compact)
            .accessibilityIdentifier("exerciseDatePicker")

            // Submit
            Button {
                Task { await viewModel.submitExercise() }
            } label: {
                Group {
                    if viewModel.submitState == .submitting {
                        ProgressView()
                            .tint(.white)
                    } else {
                        Text("Log Exercise")
                            .fontWeight(.semibold)
                    }
                }
                .frame(maxWidth: .infinity)
                .frame(height: 50)
                .background(OPColor.teal)
                .foregroundStyle(.white)
                .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            .disabled(!viewModel.exerciseIsValid || viewModel.submitState == .submitting)
            .sensoryFeedback(.success, trigger: viewModel.submitState == .success("Exercise saved"))
            .accessibilityIdentifier("saveExerciseButton")
        }
    }
}
