// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct BloodPressureEntryForm: View {
    @Bindable var viewModel: LogViewModel

    var body: some View {
        VStack(spacing: 16) {
            // Systolic / Diastolic
            HStack(spacing: 12) {
                TextField("Systolic", text: $viewModel.systolicValue)
                    .textFieldStyle(.roundedBorder)
                    .keyboardType(.numberPad)
                    .accessibilityIdentifier("systolicField")

                Text("/")
                    .font(.title2)
                    .foregroundStyle(.secondary)

                TextField("Diastolic", text: $viewModel.diastolicValue)
                    .textFieldStyle(.roundedBorder)
                    .keyboardType(.numberPad)
                    .accessibilityIdentifier("diastolicField")

                Text("mmHg")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            // Date/Time
            DatePicker(
                "Measured at",
                selection: $viewModel.bloodPressureDate,
                displayedComponents: [.date, .hourAndMinute]
            )
            .datePickerStyle(.compact)
            .accessibilityIdentifier("bloodPressureDatePicker")

            // Submit
            Button {
                Task { await viewModel.submitBloodPressure() }
            } label: {
                Group {
                    if viewModel.submitState == .submitting {
                        ProgressView()
                            .tint(.white)
                    } else {
                        Text("Log Blood Pressure")
                            .fontWeight(.semibold)
                    }
                }
                .frame(maxWidth: .infinity)
                .frame(height: 50)
                .background(OPColor.teal)
                .foregroundStyle(.white)
                .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            .disabled(!viewModel.bloodPressureIsValid || viewModel.submitState == .submitting)
            .sensoryFeedback(.success, trigger: viewModel.submitState == .success("Blood pressure saved"))
            .accessibilityIdentifier("saveBloodPressureButton")
        }
    }
}
