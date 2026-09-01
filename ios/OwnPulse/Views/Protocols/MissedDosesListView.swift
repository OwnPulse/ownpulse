// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

/// Simple review list for `GET /protocols/runs/missed-doses`, reachable from
/// the dashboard's "N missed doses — Review" row. Each row can be logged or
/// skipped directly, without navigating into the owning protocol.
struct MissedDosesListView: View {
    @Bindable var viewModel: ProtocolsViewModel

    var body: some View {
        Group {
            switch viewModel.missedDosesState {
            case .idle, .loading:
                ProgressView()
                    .frame(maxWidth: .infinity, minHeight: 200)
                    .accessibilityIdentifier("missedDosesLoading")

            case .error(let message):
                VStack(spacing: 12) {
                    Image(systemName: "exclamationmark.triangle")
                        .font(.largeTitle)
                        .foregroundStyle(OPColor.terracotta)
                    Text(message)
                        .foregroundStyle(.secondary)
                    Button("Retry") {
                        Task { await viewModel.loadMissedDoses() }
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(OPColor.terracotta)
                }
                .frame(maxWidth: .infinity, minHeight: 200)
                .accessibilityIdentifier("missedDosesError")

            case .loaded:
                if viewModel.missedDoses.isEmpty {
                    ContentUnavailableView {
                        Label("All Caught Up", systemImage: "checkmark.circle")
                    } description: {
                        Text("No missed doses across your active protocols.")
                    }
                    .accessibilityIdentifier("missedDosesEmpty")
                } else {
                    List(viewModel.missedDoses) { item in
                        row(item)
                    }
                    .accessibilityIdentifier("missedDosesList")
                }
            }
        }
        .navigationTitle("Missed Doses")
        .navigationBarTitleDisplayMode(.inline)
        .task {
            await viewModel.loadMissedDoses()
        }
    }

    @ViewBuilder
    private func row(_ item: MissedDoseItem) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(item.substance)
                .font(.subheadline)
                .fontWeight(.medium)
            Text("\(item.protocolName) · \(item.date)")
                .font(.caption)
                .foregroundStyle(.secondary)

            HStack(spacing: 12) {
                Button("Log") {
                    Task {
                        await viewModel.logDose(
                            protocolId: item.protocolId,
                            runId: item.runId,
                            lineId: item.protocolLineId,
                            dayNumber: item.dayNumber
                        )
                        await viewModel.loadMissedDoses()
                    }
                }
                .buttonStyle(.borderedProminent)
                .tint(OPColor.terracotta)
                .controlSize(.small)
                .accessibilityIdentifier("logMissedDoseButton-\(item.protocolLineId)-\(item.dayNumber)")

                Button("Skip") {
                    Task {
                        await viewModel.skipDose(
                            protocolId: item.protocolId,
                            runId: item.runId,
                            lineId: item.protocolLineId,
                            dayNumber: item.dayNumber
                        )
                        await viewModel.loadMissedDoses()
                    }
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .accessibilityIdentifier("skipMissedDoseButton-\(item.protocolLineId)-\(item.dayNumber)")
            }
        }
        .padding(.vertical, 4)
        .accessibilityIdentifier("missedDoseRow-\(item.protocolLineId)-\(item.dayNumber)")
    }
}
