// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct InterventionForm: View {
    @Bindable var viewModel: LogViewModel

    var body: some View {
        VStack(spacing: 16) {
            // Quick Pick (active protocol substances)
            if !viewModel.activeSubstances.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Quick Pick")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 8) {
                            ForEach(viewModel.activeSubstances) { item in
                                quickPickChip(item)
                            }
                        }
                    }
                }
                .accessibilityIdentifier("quickPickSection")
            }

            // Saved Medicines
            if !viewModel.savedMedicines.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Text("My Medicines")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Spacer()
                        Button {
                            Task { await viewModel.saveMedicine() }
                        } label: {
                            Image(systemName: "plus.circle")
                                .foregroundStyle(OPColor.terracotta)
                        }
                        .disabled(viewModel.substance.trimmingCharacters(in: .whitespaces).isEmpty)
                        .accessibilityIdentifier("saveMedicineButton")
                    }
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 8) {
                            ForEach(viewModel.savedMedicines) { medicine in
                                savedMedicineChip(medicine)
                            }
                        }
                    }
                }
                .accessibilityIdentifier("savedMedicinesSection")
            }

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

            // Route — includes the current value even when it isn't one of
            // the fixed options (e.g. applied from a saved-medicine/quick-pick
            // chip with a route like "subcutaneous" that isn't in `routes`),
            // otherwise the picker renders blank while the unmatched value is
            // still what gets submitted.
            Picker("Route", selection: $viewModel.route) {
                ForEach(routeOptions, id: \.self) { route in
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

            // Attribution parity: the entered substance+dose matches a
            // pending scheduled dose today — offer counting it toward the
            // protocol instead of logging a free-floating intervention.
            if let match = viewModel.matchingTodaysDose {
                Toggle("Count toward \(match.protocolName)", isOn: $viewModel.countTowardProtocol)
                    .accessibilityIdentifier("countTowardProtocolToggle")
            }

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
        .task {
            await viewModel.loadSavedMedicines()
            await viewModel.loadActiveSubstances()
            await viewModel.loadTodaysDoses()
        }
    }

    /// The fixed route list, plus the currently-selected value if it isn't
    /// already one of them — see the picker's comment above.
    private var routeOptions: [String] {
        guard !viewModel.route.isEmpty, !LogViewModel.routes.contains(viewModel.route) else {
            return LogViewModel.routes
        }
        return LogViewModel.routes + [viewModel.route]
    }

    private func quickPickChip(_ item: ActiveSubstance) -> some View {
        Button {
            viewModel.applyActiveSubstance(item)
        } label: {
            VStack(alignment: .leading, spacing: 2) {
                Text(quickPickLabel(item))
                    .font(.caption)
                Text(item.protocolName)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(Capsule().stroke(OPColor.teal, lineWidth: 1))
        }
        .accessibilityIdentifier("quickPickChip-\(item.id)")
    }

    private func quickPickLabel(_ item: ActiveSubstance) -> String {
        var parts = [item.substance]
        if let d = item.dose {
            var dosePart = LogViewModel.formatDose(d)
            if let u = item.unit { dosePart += u }
            parts.append(dosePart)
        }
        if let r = item.route { parts.append(r) }
        return parts.joined(separator: " ")
    }

    private func savedMedicineChip(_ medicine: SavedMedicine) -> some View {
        Button {
            viewModel.applySavedMedicine(medicine)
        } label: {
            Text(savedMedicineLabel(medicine))
                .font(.caption)
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .background(Capsule().stroke(OPColor.terracotta, lineWidth: 1))
        }
        .contextMenu {
            Button(role: .destructive) {
                Task { await viewModel.deleteSavedMedicine(medicine.id) }
            } label: {
                Label("Delete", systemImage: "trash")
            }
        }
        .accessibilityIdentifier("savedMedicineChip-\(medicine.id)")
    }

    private func savedMedicineLabel(_ medicine: SavedMedicine) -> String {
        var parts = [medicine.substance]
        if let d = medicine.dose {
            var dosePart = LogViewModel.formatDose(d)
            if let u = medicine.unit { dosePart += u }
            parts.append(dosePart)
        }
        if let r = medicine.route { parts.append(r) }
        return parts.joined(separator: " ")
    }
}
