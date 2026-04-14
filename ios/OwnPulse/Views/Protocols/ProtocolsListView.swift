// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct ProtocolsListView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var viewModel: ProtocolsViewModel?
    @State private var showingBuilder = false

    var body: some View {
        Group {
            if let vm = viewModel {
                listContent(vm: vm)
            } else {
                ProgressView()
                    .frame(maxWidth: .infinity, minHeight: 200)
            }
        }
        .navigationTitle("Protocols")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button("New") {
                    showingBuilder = true
                }
                .accessibilityIdentifier("newProtocolButton")
            }
        }
        .sheet(isPresented: $showingBuilder) {
            if let vm = viewModel {
                NavigationStack {
                    ProtocolBuilderView(viewModel: vm)
                }
            }
        }
        .onAppear {
            if viewModel == nil {
                viewModel = ProtocolsViewModel(networkClient: dependencies.networkClient)
            }
            Task { await viewModel?.loadProtocols() }
        }
    }

    @ViewBuilder
    private func listContent(vm: ProtocolsViewModel) -> some View {
        VStack(spacing: 0) {
            // Filter picker
            Picker("Filter", selection: Bindable(vm).filter) {
                ForEach(ProtocolsViewModel.ProtocolFilter.allCases, id: \.self) { f in
                    Text(f.rawValue).tag(f)
                }
            }
            .pickerStyle(.segmented)
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
            .accessibilityIdentifier("protocolFilterPicker")

            switch vm.listState {
            case .idle, .loading:
                ProgressView()
                    .frame(maxWidth: .infinity, minHeight: 200)
                    .accessibilityIdentifier("protocolsLoading")

            case .error(let message):
                VStack(spacing: 12) {
                    Image(systemName: "exclamationmark.triangle")
                        .font(.largeTitle)
                        .foregroundStyle(OPColor.terracotta)
                    Text(message)
                        .foregroundStyle(.secondary)
                    Button("Retry") {
                        Task { await vm.loadProtocols() }
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(OPColor.terracotta)
                }
                .frame(maxWidth: .infinity, minHeight: 200)
                .accessibilityIdentifier("protocolsError")

            case .loaded:
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 12) {
                        // Active Runs section
                        if !vm.activeRuns.isEmpty {
                            Text("Active Runs")
                                .font(.title3)
                                .fontWeight(.semibold)
                                .padding(.top, 4)

                            ForEach(vm.activeRuns) { run in
                                NavigationLink(value: run.protocolId) {
                                    ActiveRunCardView(run: run)
                                }
                                .buttonStyle(.plain)
                                .accessibilityIdentifier("activeRunCard-\(run.id)")
                            }
                        }

                        // My Protocols section
                        Text("My Protocols")
                            .font(.title3)
                            .fontWeight(.semibold)
                            .padding(.top, vm.activeRuns.isEmpty ? 4 : 12)

                        if vm.filteredProtocols.isEmpty {
                            ContentUnavailableView {
                                Label("No Protocols", systemImage: "list.bullet.clipboard")
                            } description: {
                                Text(vm.protocols.isEmpty
                                    ? "Create your first dosing protocol."
                                    : "No protocols match this filter.")
                            } actions: {
                                if vm.protocols.isEmpty {
                                    Button("New Protocol") {
                                        showingBuilder = true
                                    }
                                    .buttonStyle(.borderedProminent)
                                    .tint(OPColor.terracotta)
                                }
                            }
                            .accessibilityIdentifier("protocolsEmpty")
                        } else {
                            ForEach(vm.filteredProtocols) { proto in
                                NavigationLink(value: proto.id) {
                                    ProtocolCardView(item: proto, viewModel: vm)
                                }
                                .buttonStyle(.plain)
                                .accessibilityIdentifier("protocolCard-\(proto.id)")
                            }
                        }
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                }
                .refreshable {
                    await vm.loadProtocols()
                }
                .accessibilityIdentifier("protocolsList")
            }
        }
        .navigationDestination(for: String.self) { protocolId in
            ProtocolDetailView(protocolId: protocolId, viewModel: vm)
        }
    }
}

// MARK: - Active Run Card

private struct ActiveRunCardView: View {
    let run: ActiveRunResponse

    private var hasPending: Bool {
        run.dosesToday > 0 && run.dosesCompletedToday < run.dosesToday
    }

    private var doseBadge: String {
        if run.dosesToday == 0 { return "No doses today" }
        if run.dosesCompletedToday >= run.dosesToday { return "All done" }
        let pending = run.dosesToday - run.dosesCompletedToday
        return "\(pending) dose\(pending != 1 ? "s" : "") pending"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(run.protocolName ?? "Protocol")
                    .font(.headline)
                    .lineLimit(1)
                Spacer()
                Text(doseBadge)
                    .font(.caption)
                    .fontWeight(.semibold)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 3)
                    .background(hasPending
                        ? OPColor.terracotta.opacity(0.15)
                        : OPColor.sage.opacity(0.15))
                    .foregroundStyle(hasPending ? OPColor.terracotta : OPColor.sage)
                    .clipShape(Capsule())
            }

            ProgressView(value: run.progressPct / 100.0)
                .tint(OPColor.terracotta)

            Text("Started \(run.startDate) · \(Int(run.progressPct))% complete")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .opCard()
        .overlay(
            Rectangle()
                .fill(hasPending ? OPColor.terracotta : .clear)
                .frame(width: 3),
            alignment: .leading
        )
        .accessibilityIdentifier("activeRunCard")
    }
}

// MARK: - Protocol Card

private struct ProtocolCardView: View {
    let item: ProtocolListItem
    let viewModel: ProtocolsViewModel

    @State private var startingRun = false

    private var hasActiveRun: Bool {
        viewModel.activeRun(for: item.id) != nil
    }

    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text(item.name)
                        .font(.headline)
                        .lineLimit(1)
                    Spacer()
                    StatusBadge(status: item.status)
                }

                HStack {
                    if let nextDose = item.nextDose {
                        Text("Next: \(nextDose)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    Text("\(item.durationDays) days")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            if !hasActiveRun {
                Button {
                    startingRun = true
                    Task {
                        let success = await viewModel.startRun(protocolId: item.id)
                        startingRun = false
                        if success {
                            await viewModel.loadProtocols()
                        }
                    }
                } label: {
                    if startingRun {
                        ProgressView()
                            .controlSize(.small)
                    } else {
                        Text("Start")
                    }
                }
                .buttonStyle(.borderedProminent)
                .tint(OPColor.terracotta)
                .controlSize(.small)
                .disabled(startingRun)
                .accessibilityIdentifier("startRunButton-\(item.id)")
            }
        }
        .opCard()
    }
}

// MARK: - Status Badge

struct StatusBadge: View {
    let status: ProtocolStatus

    var body: some View {
        Text(status.rawValue.capitalized)
            .font(.caption)
            .fontWeight(.semibold)
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(badgeColor.opacity(0.15))
            .foregroundStyle(badgeColor)
            .clipShape(Capsule())
    }

    private var badgeColor: Color {
        switch status {
        case .active: return OPColor.sage
        case .paused: return OPColor.gold
        case .completed: return OPColor.teal
        case .draft: return .secondary
        case .archived: return .secondary
        }
    }
}
