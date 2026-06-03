// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "source-preference-wizard")

// MARK: - API models

/// One competing source for a metric, as returned by `/sources/overlap-scan`.
struct OverlapSource: Decodable, Sendable, Hashable {
    let source: String
    let recordCount: Int

    enum CodingKeys: String, CodingKey {
        case source
        case recordCount = "record_count"
    }
}

/// A metric (`record_type`) that has overlapping records from more than one
/// source over the scan window.
struct OverlapMetric: Decodable, Sendable, Identifiable, Hashable {
    let metricType: String
    let sources: [OverlapSource]

    var id: String { metricType }

    enum CodingKeys: String, CodingKey {
        case metricType = "metric_type"
        case sources
    }
}

/// Response body for `GET /sources/overlap-scan`.
struct OverlapScanResponse: Decodable, Sendable {
    let metrics: [OverlapMetric]
}

/// Request body for `POST /source-preferences`.
struct UpsertSourcePreferenceRequest: Encodable, Sendable {
    let metricType: String
    let preferredSource: String

    enum CodingKeys: String, CodingKey {
        case metricType = "metric_type"
        case preferredSource = "preferred_source"
    }
}

// MARK: - View model

@Observable
@MainActor
final class SourcePreferenceWizardViewModel {
    enum State: Equatable {
        case loading
        case empty
        case ready
        case saving
        case finished
        case failed(String)
    }

    private(set) var state: State = .loading
    private(set) var metrics: [OverlapMetric] = []

    /// User's chosen source per metric type. Defaults to the highest-count
    /// source for each metric once the scan loads.
    var selections: [String: String] = [:]

    private let networkClient: NetworkClientProtocol

    init(networkClient: NetworkClientProtocol) {
        self.networkClient = networkClient
    }

    /// Whether there is anything for the user to act on.
    var hasConflicts: Bool { !metrics.isEmpty }

    func loadOverlaps() async {
        state = .loading
        do {
            let response: OverlapScanResponse = try await networkClient.request(
                method: "GET",
                path: Endpoints.sourcesOverlapScan,
                body: Optional<String>.none
            )
            metrics = response.metrics
            // Pre-select the source with the most records for each metric. The
            // backend already orders sources by descending count.
            for metric in metrics where selections[metric.metricType] == nil {
                if let first = metric.sources.first {
                    selections[metric.metricType] = first.source
                }
            }
            state = metrics.isEmpty ? .empty : .ready
        } catch {
            logger.error("Failed to load source overlaps: \(error.localizedDescription, privacy: .public)")
            state = .failed("Couldn't check for source conflicts. Try again.")
        }
    }

    /// Persist every chosen preference via the existing `/source-preferences`
    /// write path. Returns when all writes complete.
    func saveSelections() async {
        guard state == .ready else { return }
        state = .saving
        do {
            for metric in metrics {
                guard let chosen = selections[metric.metricType] else { continue }
                let body = UpsertSourcePreferenceRequest(
                    metricType: metric.metricType,
                    preferredSource: chosen
                )
                let _: SourcePreferenceResponse = try await networkClient.request(
                    method: "POST",
                    path: Endpoints.sourcePreferences,
                    body: body
                )
            }
            state = .finished
        } catch {
            logger.error("Failed to save source preferences: \(error.localizedDescription, privacy: .public)")
            state = .failed("Couldn't save your choices. Try again.")
        }
    }
}

/// Minimal decode target for the `/source-preferences` POST response. We don't
/// use the fields — we only need the request to succeed.
struct SourcePreferenceResponse: Decodable, Sendable {
    let id: String
    let metricType: String
    let preferredSource: String

    enum CodingKeys: String, CodingKey {
        case id
        case metricType = "metric_type"
        case preferredSource = "preferred_source"
    }
}

// MARK: - View

/// One-screen wizard that asks the user which source should win for each metric
/// that has overlapping records from more than one connected source. Triggered
/// after a first Garmin/Oura connect, or manually from Settings.
struct SourcePreferenceWizard: View {
    @Environment(\.dismiss) private var dismiss
    @State private var viewModel: SourcePreferenceWizardViewModel

    init(networkClient: NetworkClientProtocol) {
        _viewModel = State(initialValue: SourcePreferenceWizardViewModel(networkClient: networkClient))
    }

    var body: some View {
        NavigationStack {
            content
                .navigationTitle("Source of Truth")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Close") { dismiss() }
                            .accessibilityIdentifier("sourceWizardCloseButton")
                    }
                }
        }
        .task { await viewModel.loadOverlaps() }
    }

    @ViewBuilder
    private var content: some View {
        switch viewModel.state {
        case .loading:
            ProgressView("Checking for overlaps…")
                .accessibilityIdentifier("sourceWizardLoading")
        case .empty:
            ContentUnavailableView(
                "No Conflicts",
                systemImage: "checkmark.circle",
                description: Text("None of your metrics have records from more than one source.")
            )
            .accessibilityIdentifier("sourceWizardEmpty")
        case .finished:
            ContentUnavailableView(
                "Preferences Saved",
                systemImage: "checkmark.circle.fill",
                description: Text("OwnPulse will prefer your chosen source for each metric.")
            )
            .accessibilityIdentifier("sourceWizardFinished")
        case .failed(let message):
            ContentUnavailableView {
                Label("Something Went Wrong", systemImage: "exclamationmark.triangle")
            } description: {
                Text(message)
            } actions: {
                Button("Retry") {
                    Task { await viewModel.loadOverlaps() }
                }
                .accessibilityIdentifier("sourceWizardRetryButton")
            }
            .accessibilityIdentifier("sourceWizardError")
        case .ready, .saving:
            picker
        }
    }

    private var picker: some View {
        List {
            Section {
                Text("Several metrics have records from more than one source. Choose which source OwnPulse should treat as the source of truth for each.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            ForEach(viewModel.metrics) { metric in
                Section(humanize(metric.metricType)) {
                    Picker("Source", selection: bindingForMetric(metric.metricType)) {
                        ForEach(metric.sources, id: \.source) { source in
                            Text("\(humanize(source.source)) (\(source.recordCount))")
                                .tag(source.source)
                        }
                    }
                    .pickerStyle(.inline)
                    .labelsHidden()
                    .accessibilityIdentifier("sourcePicker-\(metric.metricType)")
                }
            }

            Section {
                Button {
                    Task {
                        await viewModel.saveSelections()
                        if case .finished = viewModel.state { dismiss() }
                    }
                } label: {
                    if viewModel.state == .saving {
                        ProgressView()
                    } else {
                        Text("Save Preferences")
                    }
                }
                .disabled(viewModel.state == .saving)
                .accessibilityIdentifier("sourceWizardSaveButton")
            }
        }
    }

    private func bindingForMetric(_ metricType: String) -> Binding<String> {
        Binding(
            get: { viewModel.selections[metricType] ?? "" },
            set: { viewModel.selections[metricType] = $0 }
        )
    }

    /// Turn a snake_case metric or source identifier into a readable label.
    /// Pure formatting — never validates or filters the value.
    private func humanize(_ identifier: String) -> String {
        identifier
            .split(separator: "_")
            .map { $0.prefix(1).uppercased() + $0.dropFirst() }
            .joined(separator: " ")
    }
}
