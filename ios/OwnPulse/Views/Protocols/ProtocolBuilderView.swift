// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct ProtocolBuilderView: View {
    @Bindable var viewModel: ProtocolsViewModel
    @Environment(\.dismiss) private var dismiss

    private static let routes = ["SubQ", "IM", "Oral", "Topical", "Nasal", "IV"]
    private static let weekOptions = Array(1...52)

    var body: some View {
        Form {
            protocolHeaderSection
            linesSection
            statusSection
        }
        .navigationTitle("New Protocol")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("Cancel") {
                    viewModel.resetCreateForm()
                    dismiss()
                }
                .accessibilityIdentifier("cancelBuilderButton")
            }
            ToolbarItem(placement: .confirmationAction) {
                Button("Create") {
                    Task {
                        await viewModel.createProtocol()
                        if case .success = viewModel.createState {
                            await viewModel.loadProtocols()
                            dismiss()
                        }
                    }
                }
                .disabled(!viewModel.createIsValid || viewModel.createState == .submitting)
                .accessibilityIdentifier("createProtocolButton")
            }
        }
    }

    // MARK: - Sections

    @ViewBuilder
    private var protocolHeaderSection: some View {
        Section("Protocol") {
            TextField("Name", text: Bindable(viewModel).newName)
                .accessibilityIdentifier("protocolNameField")

            DatePicker("Start Date", selection: Bindable(viewModel).newStartDate, displayedComponents: .date)
                .accessibilityIdentifier("protocolStartDate")

            Picker("Duration", selection: Bindable(viewModel).newWeeks) {
                ForEach(Self.weekOptions, id: \.self) { w in
                    Text("\(w) \(w == 1 ? "week" : "weeks")").tag(w)
                }
            }
            .accessibilityIdentifier("protocolDurationPicker")

            TextField("Description (optional)", text: Bindable(viewModel).newDescription, axis: .vertical)
                .lineLimit(2...4)
                .accessibilityIdentifier("protocolDescriptionField")
        }
    }

    @ViewBuilder
    private var linesSection: some View {
        Section("Substances") {
            ForEach(viewModel.newLines.indices, id: \.self) { index in
                lineEditor(index: index)
            }
            .onDelete { indexSet in
                for i in indexSet {
                    viewModel.removeLine(at: i)
                }
            }

            Button("Add Substance") {
                viewModel.addLine()
            }
            .accessibilityIdentifier("addSubstanceButton")
        }
    }

    @ViewBuilder
    private func lineEditor(index: Int) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            TextField("Substance", text: $viewModel.newLines[index].substance)
                .accessibilityIdentifier("substanceName-\(index)")

            HStack {
                TextField("Dose", text: $viewModel.newLines[index].dose)
                    .keyboardType(.decimalPad)
                    .accessibilityIdentifier("substanceDose-\(index)")

                TextField("Unit", text: $viewModel.newLines[index].unit)
                    .frame(maxWidth: 80)
                    .accessibilityIdentifier("substanceUnit-\(index)")
            }

            HStack {
                Picker("Route", selection: $viewModel.newLines[index].route) {
                    Text("Select...").tag("")
                    ForEach(Self.routes, id: \.self) { r in
                        Text(r).tag(r)
                    }
                }
                .accessibilityIdentifier("substanceRoute-\(index)")

                Picker("Time", selection: $viewModel.newLines[index].timeOfDay) {
                    Text("Any").tag("")
                    Text("AM").tag("AM")
                    Text("PM").tag("PM")
                }
                .accessibilityIdentifier("substanceTime-\(index)")
            }

            Picker("Schedule", selection: $viewModel.newLines[index].patternType) {
                ForEach(PatternType.allCases, id: \.self) { p in
                    Text(p.rawValue).tag(p)
                }
            }
            .accessibilityIdentifier("substancePattern-\(index)")
        }
        .padding(.vertical, 4)
    }

    @ViewBuilder
    private var statusSection: some View {
        switch viewModel.createState {
        case .submitting:
            Section {
                HStack {
                    ProgressView()
                    Text("Creating protocol...")
                        .foregroundStyle(.secondary)
                }
            }
        case .error(let message):
            Section {
                HStack {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(.red)
                    Text(message)
                        .foregroundStyle(.red)
                        .font(.caption)
                }
                .accessibilityIdentifier("builderError")
            }
        case .idle, .success:
            EmptyView()
        }
    }
}
