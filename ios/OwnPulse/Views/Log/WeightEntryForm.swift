// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct WeightEntryForm: View {
    @Bindable var viewModel: LogViewModel

    var body: some View {
        VStack(spacing: 16) {
            // Weight + Unit
            HStack(spacing: 12) {
                TextField("Weight", text: $viewModel.weightValue)
                    .textFieldStyle(.roundedBorder)
                    .keyboardType(.decimalPad)
                    .accessibilityIdentifier("weightField")

                Picker("Unit", selection: $viewModel.weightUnit) {
                    ForEach(LogViewModel.weightUnits, id: \.self) { unit in
                        Text(unit).tag(unit)
                    }
                }
                .pickerStyle(.menu)
                .accessibilityIdentifier("weightUnitPicker")
            }

            // Date/Time
            DatePicker(
                "Measured at",
                selection: $viewModel.weightDate,
                displayedComponents: [.date, .hourAndMinute]
            )
            .datePickerStyle(.compact)
            .accessibilityIdentifier("weightDatePicker")

            // Submit
            Button {
                Task { await viewModel.submitWeight() }
            } label: {
                Group {
                    if viewModel.submitState == .submitting {
                        ProgressView()
                            .tint(.white)
                    } else {
                        Text("Log Weight")
                            .fontWeight(.semibold)
                    }
                }
                .frame(maxWidth: .infinity)
                .frame(height: 50)
                .background(OPColor.teal)
                .foregroundStyle(.white)
                .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            .disabled(!viewModel.weightIsValid || viewModel.submitState == .submitting)
            .sensoryFeedback(.success, trigger: viewModel.submitState == .success("Weight saved"))
            .accessibilityIdentifier("saveWeightButton")
        }
    }
}
