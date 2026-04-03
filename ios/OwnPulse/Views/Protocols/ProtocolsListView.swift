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
                    ScrollView {
                        LazyVStack(spacing: 12) {
                            ForEach(vm.filteredProtocols) { proto in
                                NavigationLink(value: proto.id) {
                                    ProtocolCardView(item: proto, viewModel: vm)
                                }
                                .buttonStyle(.plain)
                                .accessibilityIdentifier("protocolCard-\(proto.id)")
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
        }
        .navigationDestination(for: String.self) { protocolId in
            ProtocolDetailView(protocolId: protocolId, viewModel: vm)
        }
    }
}

// MARK: - Protocol Card

private struct ProtocolCardView: View {
    let item: ProtocolListItem
    let viewModel: ProtocolsViewModel

    var body: some View {
        let progress = viewModel.computeProgress(for: item)
        let pct = progress.total > 0
            ? Double(progress.completed) / Double(progress.total)
            : 0

        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(item.name)
                    .font(.headline)
                    .lineLimit(1)
                Spacer()
                StatusBadge(status: item.status)
            }

            ProgressView(value: pct)
                .tint(OPColor.terracotta)
                .accessibilityIdentifier("protocolProgress")

            HStack {
                Text("\(progress.completed)/\(progress.total) doses")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                Text("\(item.durationDays) days")
                    .font(.caption)
                    .foregroundStyle(.secondary)
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
        }
    }
}
