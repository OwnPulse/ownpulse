// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("WearableConnectionsViewModel", .serialized)
@MainActor
struct WearableConnectionsViewModelTests {
    private func makeKeychain(token: String? = "jwt-abc") -> MockKeychainService {
        let keychain = MockKeychainService()
        if let token {
            try? keychain.save(key: AuthService.accessTokenKey, data: Data(token.utf8))
        }
        return keychain
    }

    private func makeVM(
        network: MockNetworkClient,
        token: String? = "jwt-abc"
    ) -> WearableConnectionsViewModel {
        WearableConnectionsViewModel(
            networkClient: network,
            keychainService: makeKeychain(token: token)
        )
    }

    // MARK: - loadStatus

    @Test("loadStatus marks connected providers and transitions to loaded")
    func loadStatusSuccess() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { method, path, _ -> Any in
            #expect(method == "GET")
            #expect(path == Endpoints.integrations)
            return [
                IntegrationStatus(source: "garmin", connected: true),
                IntegrationStatus(source: "oura", connected: false),
            ]
        }
        let vm = makeVM(network: mock)

        await vm.loadStatus()

        #expect(vm.loadState == .loaded)
        #expect(vm.isConnected(.garmin))
        #expect(!vm.isConnected(.oura))
    }

    @Test("loadStatus surfaces a failure state on network error")
    func loadStatusFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ -> Any in
            throw NetworkError.serverError(statusCode: 500, body: "boom")
        }
        let vm = makeVM(network: mock)

        await vm.loadStatus()

        if case .failed = vm.loadState {
            // expected
        } else {
            Issue.record("expected failed state, got \(vm.loadState)")
        }
        #expect(!vm.isConnected(.garmin))
    }

    @Test("loadStatus treats unauthorized as a failure, not a crash")
    func loadStatusUnauthorized() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ -> Any in
            throw NetworkError.unauthorized
        }
        let vm = makeVM(network: mock)

        await vm.loadStatus()

        if case .failed = vm.loadState {} else {
            Issue.record("expected failed state, got \(vm.loadState)")
        }
    }

    // MARK: - oauthFlow / beginConnect

    @Test("oauthFlow builds the login URL with the Bearer token from Keychain")
    func oauthFlowBuildsURL() {
        let vm = makeVM(network: MockNetworkClient(), token: "jwt-xyz")

        let flow = vm.oauthFlow(for: .garmin)

        #expect(flow != nil)
        #expect(flow?.bearerToken == "jwt-xyz")
        #expect(flow?.startURL.path == Endpoints.authGarminLogin)
        #expect(flow?.apiOrigin == AppConfig.apiBaseURL)
    }

    @Test("oauthFlow returns nil when no access token is stored")
    func oauthFlowNoToken() {
        let vm = makeVM(network: MockNetworkClient(), token: nil)

        #expect(vm.oauthFlow(for: .oura) == nil)
    }

    @Test("beginConnect sets the active provider when authenticated")
    func beginConnectAuthed() {
        let vm = makeVM(network: MockNetworkClient(), token: "jwt-abc")

        vm.beginConnect(.oura)

        #expect(vm.activeProvider == .oura)
        #expect(vm.connectError == nil)
    }

    @Test("beginConnect surfaces an error and does not start when signed out")
    func beginConnectSignedOut() {
        let vm = makeVM(network: MockNetworkClient(), token: nil)

        vm.beginConnect(.garmin)

        #expect(vm.activeProvider == nil)
        #expect(vm.connectError != nil)
    }

    // MARK: - handleResult

    @Test("handleResult connected on first connect triggers the source wizard")
    func handleResultFirstConnect() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ -> Any in
            [IntegrationStatus(source: "garmin", connected: true)]
        }
        let vm = makeVM(network: mock)

        await vm.handleResult(.connected(provider: "garmin"), for: .garmin)

        #expect(vm.activeProvider == nil)
        #expect(vm.isConnected(.garmin))
        #expect(vm.shouldShowSourceWizard)
    }

    @Test("handleResult connected for an already-connected provider does not re-trigger the wizard")
    func handleResultReconnect() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ -> Any in
            [IntegrationStatus(source: "garmin", connected: true)]
        }
        let vm = makeVM(network: mock)
        await vm.loadStatus() // garmin already connected

        await vm.handleResult(.connected(provider: "garmin"), for: .garmin)

        #expect(!vm.shouldShowSourceWizard)
        #expect(vm.isConnected(.garmin))
    }

    @Test("handleResult cancelled clears the active provider without error")
    func handleResultCancelled() async {
        let vm = makeVM(network: MockNetworkClient())
        vm.beginConnect(.oura)

        await vm.handleResult(.cancelled, for: .oura)

        #expect(vm.activeProvider == nil)
        #expect(vm.connectError == nil)
        #expect(!vm.shouldShowSourceWizard)
    }

    @Test("handleResult failed surfaces the message to the user")
    func handleResultFailed() async {
        let vm = makeVM(network: MockNetworkClient())
        vm.beginConnect(.oura)

        await vm.handleResult(.failed(message: "Network down"), for: .oura)

        #expect(vm.activeProvider == nil)
        #expect(vm.connectError == "Network down")
    }

    // MARK: - disconnect

    @Test("disconnect removes the provider and reloads status")
    func disconnectSuccess() async {
        let mock = MockNetworkClient()
        var deleteCalled = false
        mock.requestNoContentHandler = { method, path, _ in
            #expect(method == "DELETE")
            #expect(path == "\(Endpoints.integrations)/garmin")
            deleteCalled = true
        }
        mock.requestHandler = { _, _, _ -> Any in
            [IntegrationStatus]() // nothing connected after delete
        }
        let vm = makeVM(network: mock)

        await vm.disconnect(.garmin)

        #expect(deleteCalled)
        #expect(!vm.isConnected(.garmin))
        #expect(vm.connectError == nil)
    }

    @Test("disconnect surfaces an error on failure")
    func disconnectFailure() async {
        let mock = MockNetworkClient()
        mock.requestNoContentHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "boom")
        }
        let vm = makeVM(network: mock)

        await vm.disconnect(.oura)

        #expect(vm.connectError != nil)
    }
}

// MARK: - OAuthWebView.Coordinator pure logic

@Suite("OAuthWebView.Coordinator")
@MainActor
struct OAuthWebViewCoordinatorTests {
    private func coordinator(
        provider: String = "garmin",
        origin: String = "https://app.ownpulse.health",
        token: String? = "jwt-abc"
    ) -> OAuthWebView.Coordinator {
        OAuthWebView.Coordinator(
            provider: provider,
            apiOrigin: URL(string: origin)!,
            bearerToken: token,
            onResult: { _ in }
        )
    }

    @Test("isSameOrigin matches scheme, host and port")
    func sameOrigin() {
        let coord = coordinator(origin: "http://localhost:8080")
        #expect(coord.isSameOrigin(URL(string: "http://localhost:8080/api/v1/auth/garmin/login")!))
        #expect(!coord.isSameOrigin(URL(string: "https://connect.garmin.com/oauthConfirm")!))
        #expect(!coord.isSameOrigin(URL(string: "http://localhost:9090/api")!))
    }

    @Test("isSuccessRedirect matches /settings?connected=<provider>")
    func successRedirect() {
        let coord = coordinator(provider: "oura", origin: "https://app.ownpulse.health")
        #expect(coord.isSuccessRedirect(URL(string: "https://app.ownpulse.health/settings?connected=oura")!))
        // Wrong provider — not this flow's success.
        #expect(!coord.isSuccessRedirect(URL(string: "https://app.ownpulse.health/settings?connected=garmin")!))
        // Right path, no query — not success.
        #expect(!coord.isSuccessRedirect(URL(string: "https://app.ownpulse.health/settings")!))
        // Cross-origin — never success.
        #expect(!coord.isSuccessRedirect(URL(string: "https://evil.example/settings?connected=oura")!))
    }

    @Test("applyAuthHeaderIfSameOrigin attaches Bearer only on same-origin requests")
    func authHeaderInjection() {
        let coord = coordinator(origin: "https://app.ownpulse.health", token: "jwt-secret")

        var same = URLRequest(url: URL(string: "https://app.ownpulse.health/api/v1/auth/garmin/login")!)
        coord.applyAuthHeaderIfSameOrigin(to: &same)
        #expect(same.value(forHTTPHeaderField: "Authorization") == "Bearer jwt-secret")

        var cross = URLRequest(url: URL(string: "https://connect.garmin.com/oauthConfirm")!)
        coord.applyAuthHeaderIfSameOrigin(to: &cross)
        #expect(cross.value(forHTTPHeaderField: "Authorization") == nil)
    }

    @Test("applyAuthHeaderIfSameOrigin is a no-op when no token is present")
    func authHeaderNoToken() {
        let coord = coordinator(token: nil)
        var req = URLRequest(url: URL(string: "https://app.ownpulse.health/api/v1/auth/garmin/login")!)
        coord.applyAuthHeaderIfSameOrigin(to: &req)
        #expect(req.value(forHTTPHeaderField: "Authorization") == nil)
    }
}
