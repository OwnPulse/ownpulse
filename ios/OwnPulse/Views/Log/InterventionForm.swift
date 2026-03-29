// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct InterventionForm: View {
    @Bindable var viewModel: LogViewModel

    var body: some View {
        VStack(spacing: 16) {
            // Substance
            TextField("Substance", text: $viewModel.substance)
                .textFieldStyle(.roundedBorder)
                .autocorrectionDisabled()
                .accessibilityIdentifier("substanceField")

            // Dose + Unit
            HStack(spacing: 12) {
                TextField("Dose", text: $viewModel.dose)
                    .textFieldStyle(.roundedBorder)
                    .keyboardType(.decimalPad)
                    .accessibilityIdentifier("doseField")

                Picker("Unit", selection: $viewModel.doseUnit) {
                    ForEach(LogViewModel.doseUnits, id: \.self) { unit in
                        Text(unit).tag(unit)
                    }
                }
                .pickerStyle(.menu)
                .accessibilityIdentifier("doseUnitPicker")
            }

            // Route
            Picker("Route", selection: $viewModel.route) {
                ForEach(LogViewModel.routes, id: \.self) { route in
                    Text(route.capitalized).tag(route)
                }
            }
            .pickerStyle(.menu)
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityIdentifier("routePicker")

            // Date/Time
            DatePicker(
                "Administered at",
                selection: $viewModel.interventionDate,
                displayedComponents: [.date, .hourAndMinute]
            )
            .datePickerStyle(.compact)
            .accessibilityIdentifier("interventionDatePicker")

            // Fasted toggle
            Toggle("Fasted", isOn: $viewModel.fasted)
                .accessibilityIdentifier("fastedToggle")

            // Notes
            TextField("Notes (optional)", text: $viewModel.interventionNotes, axis: .vertical)
                .lineLimit(2...4)
                .textFieldStyle(.roundedBorder)
                .accessibilityIdentifier("interventionNotesField")

            // Submit
            Button {
                Task { await viewModel.submitIntervention() }
            } label: {
                Group {
                    if viewModel.submitState == .submitting {
                        ProgressView()
                            .tint(.white)
                    } else {
                        Text("Log Intervention")
                            .fontWeight(.semibold)
                    }
                }
                .frame(maxWidth: .infinity)
                .frame(height: 50)
                .background(OPColor.teal)
                .foregroundStyle(.white)
                .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            .disabled(!viewModel.interventionIsValid || viewModel.submitState == .submitting)
            .sensoryFeedback(.success, trigger: viewModel.submitState == .success("Intervention logged"))
            .accessibilityIdentifier("saveInterventionButton")
        }
    }
}
