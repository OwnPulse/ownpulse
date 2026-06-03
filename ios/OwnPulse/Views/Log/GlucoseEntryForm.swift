// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct GlucoseEntryForm: View {
    @Bindable var viewModel: LogViewModel

    var body: some View {
        VStack(spacing: 16) {
            // Glucose (mg/dL — the canonical HealthKit blood-glucose unit)
            HStack(spacing: 12) {
                TextField("Glucose", text: $viewModel.glucoseValue)
                    .textFieldStyle(.roundedBorder)
                    .keyboardType(.decimalPad)
                    .accessibilityIdentifier("glucoseField")

                Text(LogViewModel.glucoseUnit)
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("glucoseUnitLabel")
            }

            // Date/Time
            DatePicker(
                "Measured at",
                selection: $viewModel.glucoseDate,
                displayedComponents: [.date, .hourAndMinute]
            )
            .datePickerStyle(.compact)
            .accessibilityIdentifier("glucoseDatePicker")

            // Submit
            Button {
                Task { await viewModel.submitGlucose() }
            } label: {
                Group {
                    if viewModel.submitState == .submitting {
                        ProgressView()
                            .tint(.white)
                    } else {
                        Text("Log Glucose")
                            .fontWeight(.semibold)
                    }
                }
                .frame(maxWidth: .infinity)
                .frame(height: 50)
                .background(OPColor.teal)
                .foregroundStyle(.white)
                .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            .disabled(!viewModel.glucoseIsValid || viewModel.submitState == .submitting)
            .sensoryFeedback(.success, trigger: viewModel.submitState == .success("Glucose saved"))
            .accessibilityIdentifier("saveGlucoseButton")
        }
    }
}
