// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct CheckinForm: View {
    @Bindable var viewModel: LogViewModel

    var body: some View {
        VStack(spacing: 20) {
            // Date picker
            DatePicker(
                "Date",
                selection: $viewModel.checkinDate,
                displayedComponents: .date
            )
            .datePickerStyle(.compact)
            .accessibilityIdentifier("checkinDatePicker")

            Divider()

            // Score sliders
            ScoreSlider(
                label: "Energy",
                value: $viewModel.energy,
                accentColor: OPColor.gold
            )

            ScoreSlider(
                label: "Mood",
                value: $viewModel.mood,
                accentColor: OPColor.terracotta
            )

            ScoreSlider(
                label: "Focus",
                value: $viewModel.focus,
                accentColor: OPColor.teal
            )

            ScoreSlider(
                label: "Recovery",
                value: $viewModel.recovery,
                accentColor: OPColor.sage
            )

            ScoreSlider(
                label: "Libido",
                value: $viewModel.libido,
                accentColor: .purple
            )

            Divider()

            // Notes
            TextField("Notes (optional)", text: $viewModel.checkinNotes, axis: .vertical)
                .lineLimit(2...4)
                .textFieldStyle(.roundedBorder)
                .accessibilityIdentifier("checkinNotesField")

            // Submit
            Button {
                Task { await viewModel.submitCheckin() }
            } label: {
                Group {
                    if viewModel.submitState == .submitting {
                        ProgressView()
                            .tint(.white)
                    } else {
                        Text("Save Check-in")
                            .fontWeight(.semibold)
                    }
                }
                .frame(maxWidth: .infinity)
                .frame(height: 50)
                .background(OPColor.terracotta)
                .foregroundStyle(.white)
                .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            .disabled(!viewModel.checkinIsValid || viewModel.submitState == .submitting)
            .sensoryFeedback(.success, trigger: viewModel.submitState == .success("Check-in saved"))
            .accessibilityIdentifier("saveCheckinButton")
        }
    }
}
