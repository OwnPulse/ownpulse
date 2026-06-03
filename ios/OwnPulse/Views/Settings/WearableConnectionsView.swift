// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "wearable-connections")

// MARK: - API model

/// One integration's connection status as returned by `GET /integrations`.
/// Mirrors the backend `IntegrationStatus` struct.
struct IntegrationStatus: Decodable, Sendable, Hashable {
    let source: String
    let connected: Bool
}

/// A provider OwnPulse can connect to via in-app OAuth.
enum WearableProvider: String, CaseIterable, Identifiable, Sendable {
    case garmin
    case oura

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .garmin: return "Garmin"
        case .oura: return "Oura"
        }
    }

    var systemImage: String {
        switch self {
        case .garmin: return "figure.run"
        case .oura: return "circle.circle"
        }
    }

    /// Backend login endpoint that starts the OAuth flow.
    var loginEndpoint: String {
        switch self {
        case .garmin: return Endpoints.authGarminLogin
        case .oura: return Endpoints.authOuraLogin
        }
    }
}

// MARK: - View model

@Observable
@MainActor
final class WearableConnectionsViewModel {
    enum LoadState: Equatable {
        case idle
        case loading
        case loaded
        case failed(String)
    }

    private(set) var loadState: LoadState = .idle
    /// Connected provider keys (e.g. `"garmin"`). Drives the per-row status.
    private(set) var connectedSources: Set<String> = []

    /// The provider whose OAuth sheet should be presented, if any.
    var activeProvider: WearableProvider?
    /// When non-nil, the source-preference wizard should be presented after a
    /// first successful connect.
    var shouldShowSourceWizard = false
    /// User-facing connect error (safe to display — never contains tokens).
    var connectError: String?

    private let networkClient: NetworkClientProtocol
    private let keychainService: KeychainServiceProtocol

    init(networkClient: NetworkClientProtocol, keychainService: KeychainServiceProtocol) {
        self.networkClient = networkClient
        self.keychainService = keychainService
    }

    func isConnected(_ provider: WearableProvider) -> Bool {
        connectedSources.contains(provider.rawValue)
    }

    /// Load the current connection status for all integrations.
    func loadStatus() async {
        loadState = .loading
        do {
            let statuses: [IntegrationStatus] = try await networkClient.request(
                method: "GET",
                path: Endpoints.integrations,
                body: Optional<String>.none
            )
            connectedSources = Set(statuses.filter(\.connected).map(\.source))
            loadState = .loaded
        } catch {
            logger.error("Failed to load integration status: \(error.localizedDescription, privacy: .public)")
            loadState = .failed("Couldn't load connection status.")
        }
    }

    /// The absolute backend URL the OAuth web view should start at, plus the
    /// API origin and Bearer token used to authenticate same-origin requests.
    /// Returns `nil` when no access token is available — the caller surfaces an
    /// auth error rather than starting a doomed flow.
    func oauthFlow(for provider: WearableProvider)
        -> (startURL: URL, apiOrigin: URL, bearerToken: String)? {
        guard let token = bearerToken() else {
            logger.error("No access token available for OAuth flow")
            return nil
        }
        let origin = AppConfig.apiBaseURL
        let start = origin.appendingPathComponent(provider.loginEndpoint)
        return (start, origin, token)
    }

    func beginConnect(_ provider: WearableProvider) {
        connectError = nil
        guard bearerToken() != nil else {
            connectError = "You need to be signed in to connect \(provider.displayName)."
            return
        }
        activeProvider = provider
    }

    /// Handle the terminal result of an OAuth web flow.
    func handleResult(_ result: OAuthWebResult, for provider: WearableProvider) async {
        activeProvider = nil
        switch result {
        case .connected(let connectedProvider):
            let wasConnectedBefore = connectedSources.contains(connectedProvider)
            connectedSources.insert(connectedProvider)
            await loadStatus()
            // First connect for this provider → offer the source-of-truth
            // wizard so the user can resolve overlaps with Apple Health.
            if !wasConnectedBefore {
                shouldShowSourceWizard = true
            }
        case .cancelled:
            break
        case .failed(let message):
            connectError = message
        }
    }

    func disconnect(_ provider: WearableProvider) async {
        connectError = nil
        do {
            try await networkClient.requestNoContent(
                method: "DELETE",
                path: "\(Endpoints.integrations)/\(provider.rawValue)",
                body: Optional<String>.none
            )
            connectedSources.remove(provider.rawValue)
            await loadStatus()
        } catch {
            logger.error("Failed to disconnect \(provider.rawValue, privacy: .public): \(error.localizedDescription, privacy: .public)")
            connectError = "Couldn't disconnect \(provider.displayName). Try again."
        }
    }

    /// Reads the JWT access token from the Keychain. Never logged or persisted
    /// elsewhere.
    private func bearerToken() -> String? {
        guard let data = try? keychainService.load(key: AuthService.accessTokenKey),
              let token = String(data: data, encoding: .utf8),
              !token.isEmpty else {
            return nil
        }
        return token
    }
}

// MARK: - View

/// Settings section that lets the user connect or disconnect wearable
/// integrations (Garmin, Oura) via an in-app OAuth flow.
struct WearableConnectionsView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var viewModel: WearableConnectionsViewModel?

    /// Invoked after a first successful connect so the host can present the
    /// shared source-preference wizard (reused from C4).
    let onFirstConnect: () -> Void

    init(onFirstConnect: @escaping () -> Void = {}) {
        self.onFirstConnect = onFirstConnect
    }

    var body: some View {
        Group {
            if let vm = viewModel {
                content(vm: vm)
                    .onChange(of: vm.shouldShowSourceWizard) { _, newValue in
                        if newValue {
                            vm.shouldShowSourceWizard = false
                            onFirstConnect()
                        }
                    }
            } else {
                ProgressView()
            }
        }
        .onAppear {
            if viewModel == nil {
                viewModel = WearableConnectionsViewModel(
                    networkClient: dependencies.networkClient,
                    keychainService: dependencies.keychainService
                )
                Task { await viewModel?.loadStatus() }
            }
        }
    }

    @ViewBuilder
    private func content(vm: WearableConnectionsViewModel) -> some View {
        ForEach(WearableProvider.allCases) { provider in
            providerRow(provider, vm: vm)
        }

        if let error = vm.connectError {
            Text(error)
                .font(.caption)
                .foregroundStyle(.red)
                .accessibilityIdentifier("wearableConnectError")
        }

        Text("Connect a wearable to import its data. Nothing is imported until you start a sync, and you can disconnect at any time.")
            .font(.caption)
            .foregroundStyle(.secondary)
    }

    @ViewBuilder
    private func providerRow(_ provider: WearableProvider, vm: WearableConnectionsViewModel) -> some View {
        let connected = vm.isConnected(provider)
        HStack {
            Image(systemName: connected ? "checkmark.circle.fill" : provider.systemImage)
                .foregroundStyle(connected ? OPColor.sage : .secondary)
                .frame(width: 24)
            VStack(alignment: .leading, spacing: 2) {
                Text(provider.displayName)
                Text(connected ? "Connected" : "Not Connected")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .accessibilityIdentifier("wearableStatus-\(provider.rawValue)")
            Spacer()
            if connected {
                Button("Disconnect", role: .destructive) {
                    Task { await vm.disconnect(provider) }
                }
                .accessibilityIdentifier("disconnect-\(provider.rawValue)")
            } else {
                Button("Connect") {
                    vm.beginConnect(provider)
                }
                .accessibilityIdentifier("connect-\(provider.rawValue)")
            }
        }
        .sheet(
            isPresented: Binding(
                get: { vm.activeProvider == provider },
                set: { if !$0 && vm.activeProvider == provider { vm.activeProvider = nil } }
            )
        ) {
            oauthSheet(for: provider, vm: vm)
        }
    }

    @ViewBuilder
    private func oauthSheet(for provider: WearableProvider, vm: WearableConnectionsViewModel) -> some View {
        NavigationStack {
            Group {
                if let flow = vm.oauthFlow(for: provider) {
                    OAuthWebView(
                        provider: provider.rawValue,
                        startURL: flow.startURL,
                        apiOrigin: flow.apiOrigin,
                        bearerToken: flow.bearerToken
                    ) { result in
                        Task { await vm.handleResult(result, for: provider) }
                    }
                } else {
                    ContentUnavailableView(
                        "Sign In Required",
                        systemImage: "person.crop.circle.badge.exclamationmark",
                        description: Text("Sign in again to connect \(provider.displayName).")
                    )
                    .accessibilityIdentifier("oauthAuthRequired-\(provider.rawValue)")
                }
            }
            .navigationTitle("Connect \(provider.displayName)")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        vm.activeProvider = nil
                    }
                    .accessibilityIdentifier("oauthCancelButton-\(provider.rawValue)")
                }
            }
        }
    }
}
