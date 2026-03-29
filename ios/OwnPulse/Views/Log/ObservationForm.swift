// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct ObservationForm: View {
    @Bindable var viewModel: LogViewModel

    var body: some View {
        VStack(spacing: 16) {
            // Type picker
            Picker("Type", selection: $viewModel.observationType) {
                ForEach(ObservationType.allCases, id: \.self) { type in
                    Text(type.displayName).tag(type)
                }
            }
            .pickerStyle(.menu)
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityIdentifier("observationTypePicker")

            // Name
            TextField("Name", text: $viewModel.observationName)
                .textFieldStyle(.roundedBorder)
                .autocorrectionDisabled()
                .accessibilityIdentifier("observationNameField")

            // Date
            DatePicker(
                "Date/Time",
                selection: $viewModel.observationDate,
                displayedComponents: [.date, .hourAndMinute]
            )
            .datePickerStyle(.compact)
            .accessibilityIdentifier("observationDatePicker")

            // Type-specific fields
            typeSpecificFields

            // Submit
            Button {
                Task { await viewModel.submitObservation() }
            } label: {
                Group {
                    if viewModel.submitState == .submitting {
                        ProgressView()
                            .tint(.white)
                    } else {
                        Text("Log Observation")
                            .fontWeight(.semibold)
                    }
                }
                .frame(maxWidth: .infinity)
                .frame(height: 50)
                .background(OPColor.sage)
                .foregroundStyle(.white)
                .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            .disabled(!viewModel.observationIsValid || viewModel.submitState == .submitting)
            .sensoryFeedback(.success, trigger: viewModel.submitState == .success("Observation logged"))
            .accessibilityIdentifier("saveObservationButton")
        }
    }

    @ViewBuilder
    private var typeSpecificFields: some View {
        switch viewModel.observationType {
        case .eventDuration:
            DatePicker(
                "End Time",
                selection: $viewModel.observationEndDate,
                displayedComponents: [.date, .hourAndMinute]
            )
            .datePickerStyle(.compact)
            .accessibilityIdentifier("observationEndDatePicker")

            TextField("Notes (optional)", text: $viewModel.observationNotes, axis: .vertical)
                .lineLimit(2...4)
                .textFieldStyle(.roundedBorder)
                .accessibilityIdentifier("observationNotesField")

        case .scale:
            ScoreSlider(
                label: "Value",
                value: $viewModel.scaleValue,
                range: 1...viewModel.scaleMax,
                accentColor: OPColor.teal
            )

            Stepper("Scale max: \(viewModel.scaleMax)", value: $viewModel.scaleMax, in: 2...100)
                .accessibilityIdentifier("scaleMaxStepper")

        case .symptom:
            ScoreSlider(
                label: "Severity",
                value: $viewModel.symptomSeverity,
                accentColor: OPColor.terracotta
            )

        case .note:
            TextField("Note text", text: $viewModel.noteText, axis: .vertical)
                .lineLimit(3...8)
                .textFieldStyle(.roundedBorder)
                .accessibilityIdentifier("noteTextField")

        case .environmental:
            HStack(spacing: 12) {
                TextField("Value", text: $viewModel.environmentalValue)
                    .textFieldStyle(.roundedBorder)
                    .keyboardType(.decimalPad)
                    .accessibilityIdentifier("environmentalValueField")

                TextField("Unit", text: $viewModel.environmentalUnit)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 100)
                    .accessibilityIdentifier("environmentalUnitField")
            }

        case .eventInstant, .contextTag:
            TextField("Notes (optional)", text: $viewModel.observationNotes, axis: .vertical)
                .lineLimit(2...4)
                .textFieldStyle(.roundedBorder)
                .accessibilityIdentifier("observationNotesField")
        }
    }
}
