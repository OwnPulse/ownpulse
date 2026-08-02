// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import HealthKit
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
        inFlightIDs.insert(item.id)
        defer { inFlightIDs.remove(item.id) }
        actionError = nil

        guard let mapping = HealthKitTypeMap.mapping(forRecordType: item.hkType) else {
            await reportDeterministicFailure(item, reason: "Unknown HealthKit type: \(item.hkType)")
            return
        }

        // No route-level validation on the backend guarantees a numeric
        // value — a record enqueued without one serves `value.value == nil`.
        // There's nothing to write in that case.
        guard let numericValue = item.value.value else {
            await reportDeterministicFailure(item, reason: "Write-queue item has no numeric value")
            return
        }

        // Validates writability, unit parseability/compatibility, and
        // start/end ordering WITHOUT touching HealthKit — see
        // `HealthKitWriteBackValidator`. Category/read-only mappings (e.g.
        // sleep_analysis) and unwritable quantity types resolve `.invalid`
        // here rather than reaching `writeSample`.
        let unit: HKUnit
        let start: Date
        let end: Date
        switch HealthKitWriteBackValidator.resolve(payload: item.value, mapping: mapping) {
        case .invalid(let reason):
            await reportDeterministicFailure(item, reason: reason)
            return
        case .ready(let resolvedUnit, let resolvedStart, let resolvedEnd):
            unit = resolvedUnit
            start = resolvedStart
            end = resolvedEnd
        }

        do {
            try await healthKitProvider.writeSample(
                type: mapping.hkType,
                value: numericValue,
                unit: unit,
                start: start,
                end: end,
                syncIdentifier: item.id
            )
            try await acknowledge(item.id)
            items.removeAll { $0.id == item.id }
        } catch {
            logger.error("Confirm write-back failed: \(error.localizedDescription, privacy: .public)")
            // Reporting a failure permanently retires the item server-side
            // (see `WriteBackFailureClassifier`) — only do that for failures
            // we're sure won't clear up on their own. A transient one (e.g.
            // device locked) is left pending; the item stays in `items` and
            // the user (or the next background sync) can retry it.
            if WriteBackFailureClassifier.isDeterministic(error) {
                await reportDeterministicFailure(item, reason: error.localizedDescription)
            } else {
                actionError = "Couldn't write to Apple Health right now. Try again."
            }
        }
    }

    /// Reports `item` as permanently failed (`failures`, not `ids`) and, on
    /// success, drops it from `items` — the server has already retired it,
    /// so leaving it in the list would be stale. The user-facing message is
    /// deliberately different from a transient failure: this item will
    /// never be retried, so "try again" would be misleading.
    private func reportDeterministicFailure(_ item: HealthKitWriteQueueItem, reason: String) async {
        do {
            try await networkClient.requestNoContent(
                method: "POST",
                path: Endpoints.healthKitConfirm,
                body: HealthKitConfirm(ids: [], failures: [HealthKitConfirmFailure(id: item.id, error: reason)])
            )
            items.removeAll { $0.id == item.id }
            actionError = "This item couldn't be written to Apple Health and won't be retried."
        } catch {
            // Best-effort — if reporting the failure also fails, leave the
            // item pending so the next load/sync retries reporting it.
            actionError = "Couldn't update the queue. Try again."
        }
    }

    /// Tell the server the user chose NOT to write this item into Apple
    /// Health. This reports the item via `failures` (not `ids`) — sending it
    /// as `ids` would tell the server the write succeeded, which is false: no
    /// sample was ever written. `failures` still drops the item out of the
    /// server's pending set, same as a real write failure.
    func deny(_ item: HealthKitWriteQueueItem) async {
        guard !inFlightIDs.contains(item.id) else { return }
        actionError = nil
        inFlightIDs.insert(item.id)
        defer { inFlightIDs.remove(item.id) }

        do {
            try await networkClient.requestNoContent(
                method: "POST",
                path: Endpoints.healthKitConfirm,
                body: HealthKitConfirm(ids: [], failures: [HealthKitConfirmFailure(id: item.id, error: "declined by user")])
            )
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

    /// Accessibility identifier the `content(vm:)` switch renders for a given
    /// state. Extracted as a pure function so the state → identifier mapping is
    /// unit-testable without a simulator (the codebase has no ViewInspector).
    static func contentIdentifier(state: WriteBackQueueState, isEmpty: Bool) -> String {
        switch state {
        case .idle, .loading:
            return "writeBackLoading"
        case .error:
            return "writeBackError"
        case .loaded:
            return isEmpty ? "writeBackEmpty" : "writeBackList"
        }
    }

    /// Format a write-back value: integers render without a decimal ("72"),
    /// fractional values to two places ("72.50"). Pure + testable.
    static func formattedValue(_ value: Double) -> String {
        value == value.rounded()
            ? String(format: "%.0f", value)
            : String(format: "%.2f", value)
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
                .accessibilityIdentifier("writeBackList")

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
                Text(item.value.value.map(Self.formattedValue) ?? "—")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                // The measurement time (`value.startTime`), not the queue's
                // `scheduledAt` — those can differ, and what the user cares
                // about is when the sample was actually taken.
                Text(item.value.startTime, style: .date)
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
}
