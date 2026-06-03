// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "writeback-queue")

/// Loading/loaded/error states for the write-back queue list.
enum WriteBackQueueState: Equatable {
    case idle
    case loading
    case loaded
    case error(String)
}

/// Drives the HealthKit write-back queue screen: lists items the backend wants
/// mirrored into Apple Health and lets the user confirm (write the sample and
/// acknowledge it) or deny (acknowledge without writing) each one.
///
/// Reuses the existing `healthKitWriteQueue` (GET) and `healthKitConfirm`
/// (POST) endpoints — confirming and denying both acknowledge the item so the
/// server stops offering it; the difference is whether the sample is written
/// to HealthKit first.
@Observable
@MainActor
final class WriteBackQueueViewModel {
    private(set) var state: WriteBackQueueState = .idle
    private(set) var items: [HealthKitWriteQueueItem] = []
    /// IDs currently being processed — used to disable per-row buttons.
    private(set) var inFlightIDs: Set<String> = []
    var actionError: String?

    private let networkClient: NetworkClientProtocol
    private let healthKitProvider: HealthKitProviderProtocol

    init(
        networkClient: NetworkClientProtocol,
        healthKitProvider: HealthKitProviderProtocol
    ) {
        self.networkClient = networkClient
        self.healthKitProvider = healthKitProvider
    }

    func load() async {
        state = .loading
        actionError = nil
        do {
            items = try await networkClient.request(
                method: "GET",
                path: Endpoints.healthKitWriteQueue,
                body: Optional<String>.none
            )
            state = .loaded
        } catch {
            logger.error("Failed to load write-back queue: \(error.localizedDescription, privacy: .public)")
            state = .error("Couldn't load pending write-backs. Pull to retry.")
        }
    }

    /// Write the sample into Apple Health, then acknowledge it to the server.
    func confirm(_ item: HealthKitWriteQueueItem) async {
        guard !inFlightIDs.contains(item.id) else { return }
        guard let mapping = HealthKitTypeMap.mapping(forRecordType: item.hkType) else {
            actionError = "Unsupported data type — can't write to Apple Health."
            return
        }

        actionError = nil
        inFlightIDs.insert(item.id)
        defer { inFlightIDs.remove(item.id) }

        do {
            try await healthKitProvider.writeSample(
                type: mapping.hkType,
                value: item.value,
                unit: mapping.unit,
                start: item.scheduledAt,
                end: item.scheduledAt
            )
            try await acknowledge(item.id)
            items.removeAll { $0.id == item.id }
        } catch {
            logger.error("Confirm write-back failed: \(error.localizedDescription, privacy: .public)")
            actionError = "Couldn't write to Apple Health. Try again."
        }
    }

    /// Acknowledge the item to the server WITHOUT writing it to Apple Health.
    func deny(_ item: HealthKitWriteQueueItem) async {
        guard !inFlightIDs.contains(item.id) else { return }
        actionError = nil
        inFlightIDs.insert(item.id)
        defer { inFlightIDs.remove(item.id) }

        do {
            try await acknowledge(item.id)
            items.removeAll { $0.id == item.id }
        } catch {
            logger.error("Deny write-back failed: \(error.localizedDescription, privacy: .public)")
            actionError = "Couldn't update the queue. Try again."
        }
    }

    private func acknowledge(_ id: String) async throws {
        try await networkClient.requestNoContent(
            method: "POST",
            path: Endpoints.healthKitConfirm,
            body: HealthKitConfirm(ids: [id])
        )
    }

    /// Human-readable name for an `hk_type` record type, e.g. "Resting Heart Rate".
    func displayName(for item: HealthKitWriteQueueItem) -> String {
        item.hkType
            .replacingOccurrences(of: "_", with: " ")
            .capitalized
    }
}

// MARK: - WriteBackQueueView

struct WriteBackQueueView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var viewModel: WriteBackQueueViewModel?

    var body: some View {
        List {
            if let vm = viewModel {
                content(vm: vm)
            }
        }
        .navigationTitle("Write-Back Queue")
        .refreshable {
            await viewModel?.load()
        }
        .onAppear {
            if viewModel == nil {
                viewModel = WriteBackQueueViewModel(
                    networkClient: dependencies.networkClient,
                    healthKitProvider: dependencies.healthKitProvider
                )
            }
            Task { await viewModel?.load() }
        }
    }

    @ViewBuilder
    private func content(vm: WriteBackQueueViewModel) -> some View {
        switch vm.state {
        case .idle, .loading:
            HStack {
                Spacer()
                ProgressView()
                    .accessibilityIdentifier("writeBackLoading")
                Spacer()
            }
        case .error(let message):
            Text(message)
                .foregroundStyle(.secondary)
                .accessibilityIdentifier("writeBackError")
        case .loaded:
            if vm.items.isEmpty {
                Text("No pending write-backs. Data flows to Apple Health automatically after each sync.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .accessibilityIdentifier("writeBackEmpty")
            } else {
                Section {
                    ForEach(vm.items) { item in
                        row(vm: vm, item: item)
                    }
                } footer: {
                    Text("Confirm to write the value into Apple Health, or deny to skip it. Either way the item is cleared from the queue; your data on the server is untouched.")
                }

                if let error = vm.actionError {
                    Text(error)
                        .font(.caption)
                        .foregroundStyle(.red)
                        .accessibilityIdentifier("writeBackActionError")
                }
            }
        }
    }

    @ViewBuilder
    private func row(vm: WriteBackQueueViewModel, item: HealthKitWriteQueueItem) -> some View {
        let isBusy = vm.inFlightIDs.contains(item.id)
        VStack(alignment: .leading, spacing: 6) {
            Text(vm.displayName(for: item))
                .font(.body)
            HStack(spacing: 8) {
                Text(formattedValue(item.value))
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                Text(item.scheduledAt, style: .date)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            HStack(spacing: 12) {
                Button {
                    Task { await vm.confirm(item) }
                } label: {
                    Label("Confirm", systemImage: "checkmark.circle")
                }
                .buttonStyle(.borderedProminent)
                .disabled(isBusy)
                .accessibilityIdentifier("confirmWriteBack-\(item.id)")

                Button(role: .destructive) {
                    Task { await vm.deny(item) }
                } label: {
                    Label("Deny", systemImage: "xmark.circle")
                }
                .buttonStyle(.bordered)
                .disabled(isBusy)
                .accessibilityIdentifier("denyWriteBack-\(item.id)")

                if isBusy {
                    ProgressView()
                }
            }
        }
        .padding(.vertical, 4)
    }

    private func formattedValue(_ value: Double) -> String {
        value == value.rounded()
            ? String(format: "%.0f", value)
            : String(format: "%.2f", value)
    }
}
