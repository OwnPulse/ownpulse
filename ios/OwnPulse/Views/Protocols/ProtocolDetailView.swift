// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct ProtocolDetailView: View {
    let protocolId: String
    @Bindable var viewModel: ProtocolsViewModel
    @Environment(\.dismiss) private var dismiss
    @State private var showingDeleteConfirmation = false
    @State private var showingEdit = false

    var body: some View {
        Group {
            switch viewModel.detailState {
            case .idle, .loading:
                ProgressView()
                    .frame(maxWidth: .infinity, minHeight: 200)
                    .accessibilityIdentifier("protocolDetailLoading")

            case .error(let message):
                VStack(spacing: 12) {
                    Image(systemName: "exclamationmark.triangle")
                        .font(.largeTitle)
                        .foregroundStyle(OPColor.terracotta)
                    Text(message)
                        .foregroundStyle(.secondary)
                    Button("Retry") {
                        Task { await viewModel.loadProtocol(id: protocolId) }
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(OPColor.terracotta)
                }
                .frame(maxWidth: .infinity, minHeight: 200)
                .accessibilityIdentifier("protocolDetailError")

            case .loaded:
                if let proto = viewModel.selectedProtocol {
                    detailContent(proto)
                }
            }
        }
        .navigationTitle(viewModel.selectedProtocol?.name ?? "Protocol")
        .navigationBarTitleDisplayMode(.large)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button("Edit") {
                    showingEdit = true
                }
                .accessibilityIdentifier("editProtocolButton")
            }
            ToolbarItem(placement: .destructiveAction) {
                Button("Delete", role: .destructive) {
                    showingDeleteConfirmation = true
                }
                .accessibilityIdentifier("deleteProtocolButton")
            }
        }
        .sheet(isPresented: $showingEdit) {
            if let proto = viewModel.selectedProtocol {
                NavigationStack {
                    ProtocolEditView(
                        protocolId: proto.id,
                        initialName: proto.name,
                        initialDescription: proto.description ?? "",
                        initialStatus: proto.status,
                        viewModel: viewModel
                    )
                }
            }
        }
        .confirmationDialog(
            "Delete Protocol",
            isPresented: $showingDeleteConfirmation,
            titleVisibility: .visible
        ) {
            Button("Delete", role: .destructive) {
                Task {
                    let success = await viewModel.deleteProtocol(id: protocolId)
                    if success {
                        await viewModel.loadProtocols()
                        dismiss()
                    }
                }
            }
        } message: {
            Text("This will permanently delete the protocol and all dose records.")
        }
        .task {
            await viewModel.loadProtocol(id: protocolId)
        }
    }

    @ViewBuilder
    private func detailContent(_ proto: ProtocolDetail) -> some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 16) {
                // Status and meta
                headerSection(proto)

                // Progress
                progressSection(proto)

                // Today's Doses
                todaysDosesSection(proto)

                // Lines summary
                linesSection(proto)

                // Description
                if let desc = proto.description, !desc.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Description")
                            .font(.headline)
                        Text(desc)
                            .foregroundStyle(.secondary)
                    }
                    .opCard()
                    .accessibilityIdentifier("protocolDescription")
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
        }
        .refreshable {
            await viewModel.loadProtocol(id: protocolId)
        }
    }

    @ViewBuilder
    private func headerSection(_ proto: ProtocolDetail) -> some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text("Started \(proto.startDate)")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                Text("\(proto.durationDays) days")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            StatusBadge(status: proto.status)
        }
        .opCard()
        .accessibilityIdentifier("protocolHeader")
    }

    @ViewBuilder
    private func progressSection(_ proto: ProtocolDetail) -> some View {
        let progress = computeDetailProgress(proto)
        let pct = progress.total > 0
            ? Double(progress.completed) / Double(progress.total)
            : 0

        VStack(alignment: .leading, spacing: 8) {
            Text("Progress")
                .font(.headline)
            ProgressView(value: pct)
                .tint(OPColor.terracotta)
            Text("\(progress.completed)/\(progress.total) doses completed (\(Int(pct * 100))%)")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .opCard()
        .accessibilityIdentifier("protocolProgress")
    }

    @ViewBuilder
    private func todaysDosesSection(_ proto: ProtocolDetail) -> some View {
        let todayDoses = computeTodaysDoses(proto)

        VStack(alignment: .leading, spacing: 8) {
            Text("Today's Doses")
                .font(.headline)

            if todayDoses.isEmpty {
                Text("No doses scheduled for today.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(todayDoses, id: \.lineId) { doseInfo in
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(doseInfo.substance)
                                .font(.subheadline)
                                .fontWeight(.medium)
                            if !doseInfo.doseLabel.isEmpty {
                                Text(doseInfo.doseLabel)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        Spacer()

                        if doseInfo.status == .pending {
                            let runId = viewModel.activeRun(for: proto.id)?.id
                            Button("Log") {
                                Task {
                                    await viewModel.logDose(
                                        protocolId: proto.id,
                                        runId: runId,
                                        lineId: doseInfo.lineId,
                                        dayNumber: doseInfo.dayNumber
                                    )
                                }
                            }
                            .buttonStyle(.borderedProminent)
                            .tint(OPColor.terracotta)
                            .controlSize(.small)
                            .accessibilityIdentifier("logDoseButton-\(doseInfo.lineId)")

                            Button("Skip") {
                                Task {
                                    await viewModel.skipDose(
                                        protocolId: proto.id,
                                        runId: runId,
                                        lineId: doseInfo.lineId,
                                        dayNumber: doseInfo.dayNumber
                                    )
                                }
                            }
                            .buttonStyle(.bordered)
                            .controlSize(.small)
                            .accessibilityIdentifier("skipDoseButton-\(doseInfo.lineId)")
                        } else {
                            Text(doseInfo.status.rawValue.capitalized)
                                .font(.caption)
                                .fontWeight(.semibold)
                                .foregroundStyle(
                                    doseInfo.status == .completed ? OPColor.sage : .secondary
                                )
                        }
                    }
                    .padding(.vertical, 4)

                    if doseInfo.lineId != todayDoses.last?.lineId {
                        Divider()
                    }
                }
            }
        }
        .opCard()
        .accessibilityIdentifier("todaysDoses")
    }

    @ViewBuilder
    private func linesSection(_ proto: ProtocolDetail) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Substances")
                .font(.headline)

            ForEach(proto.lines) { line in
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(line.substance)
                            .font(.subheadline)
                            .fontWeight(.medium)
                        HStack(spacing: 8) {
                            if let dose = line.dose, let unit = line.unit {
                                Text("\(dose, specifier: "%.1f") \(unit)")
                            }
                            if let route = line.route {
                                Text(route)
                            }
                            if let time = line.timeOfDay {
                                Text(time)
                            }
                        }
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    }
                    Spacer()
                    let scheduledDays = line.schedulePattern.filter { $0 }.count
                    Text("\(scheduledDays) days")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .padding(.vertical, 4)

                if line.id != proto.lines.last?.id {
                    Divider()
                }
            }
        }
        .opCard()
        .accessibilityIdentifier("protocolLines")
    }

    // MARK: - Helpers

    private func computeDetailProgress(_ proto: ProtocolDetail) -> (completed: Int, total: Int) {
        var completed = 0
        var total = 0
        for line in proto.lines {
            for day in 0..<proto.durationDays {
                guard day < line.schedulePattern.count, line.schedulePattern[day] else { continue }
                total += 1
                if let dose = line.doses.first(where: { $0.dayNumber == day }),
                   dose.status == .completed {
                    completed += 1
                }
            }
        }
        return (completed, total)
    }

    private struct TodayDoseInfo {
        let lineId: String
        let substance: String
        let doseLabel: String
        let dayNumber: Int
        let status: DoseStatus
    }

    private func computeTodaysDoses(_ proto: ProtocolDetail) -> [TodayDoseInfo] {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        formatter.locale = Locale(identifier: "en_US_POSIX")
        guard let startDate = formatter.date(from: proto.startDate) else { return [] }
        let dayNumber = Calendar.current.dateComponents([.day], from: startDate, to: Date()).day ?? 0
        guard dayNumber >= 0, dayNumber < proto.durationDays else { return [] }

        return proto.lines.compactMap { line -> TodayDoseInfo? in
            guard dayNumber < line.schedulePattern.count, line.schedulePattern[dayNumber] else {
                return nil
            }
            let dose = line.doses.first(where: { $0.dayNumber == dayNumber })
            let status = dose?.status ?? .pending
            var label = ""
            if let d = line.dose, let u = line.unit {
                label = String(format: "%.1f", d) + " \(u)"
                if let route = line.route {
                    label += " \(route)"
                }
            }
            return TodayDoseInfo(
                lineId: line.id,
                substance: line.substance,
                doseLabel: label,
                dayNumber: dayNumber,
                status: status
            )
        }
    }
}

// MARK: - Edit View

struct ProtocolEditView: View {
    let protocolId: String
    @State var name: String
    @State var description: String
    @State var status: ProtocolStatus
    @Bindable var viewModel: ProtocolsViewModel
    @Environment(\.dismiss) private var dismiss
    @State private var saving = false

    private static let editableStatuses: [ProtocolStatus] = [.draft, .active, .paused, .completed, .archived]

    init(protocolId: String, initialName: String, initialDescription: String, initialStatus: ProtocolStatus, viewModel: ProtocolsViewModel) {
        self.protocolId = protocolId
        self._name = State(initialValue: initialName)
        self._description = State(initialValue: initialDescription)
        self._status = State(initialValue: initialStatus)
        self._viewModel = Bindable(viewModel)
    }

    var body: some View {
        Form {
            Section("Details") {
                TextField("Name", text: $name)
                    .accessibilityIdentifier("editProtocolName")
                TextField("Description", text: $description, axis: .vertical)
                    .lineLimit(2...4)
                    .accessibilityIdentifier("editProtocolDescription")
            }

            Section("Status") {
                Picker("Status", selection: $status) {
                    ForEach(Self.editableStatuses, id: \.self) { s in
                        Text(s.rawValue.capitalized).tag(s)
                    }
                }
                .pickerStyle(.segmented)
                .accessibilityIdentifier("editProtocolStatus")
            }
        }
        .navigationTitle("Edit Protocol")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("Cancel") { dismiss() }
            }
            ToolbarItem(placement: .confirmationAction) {
                Button("Save") {
                    saving = true
                    Task {
                        let success = await viewModel.updateProtocol(
                            id: protocolId,
                            name: name.trimmingCharacters(in: .whitespaces),
                            description: description.isEmpty ? nil : description,
                            status: status.rawValue
                        )
                        saving = false
                        if success {
                            await viewModel.loadProtocol(id: protocolId)
                            await viewModel.loadProtocols()
                            dismiss()
                        }
                    }
                }
                .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty || saving)
                .accessibilityIdentifier("saveProtocolButton")
            }
        }
    }
}
