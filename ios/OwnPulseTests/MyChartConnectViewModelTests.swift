// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

/// A `URLProtocol` stub that serves a fixed response for the SMART
/// `.well-known/smart-configuration` discovery request.
final class SmartConfigStubProtocol: URLProtocol, @unchecked Sendable {
    nonisolated(unsafe) static var statusCode = 200
    nonisolated(unsafe) static var body = Data()

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let response = HTTPURLResponse(
            url: request.url!,
            statusCode: Self.statusCode,
            httpVersion: nil,
            headerFields: ["Content-Type": "application/json"]
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Self.body)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}

    static func session() -> URLSession {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [SmartConfigStubProtocol.self]
        return URLSession(configuration: config)
    }
}

@Suite("MyChartConnectViewModel", .serialized)
@MainActor
struct MyChartConnectViewModelTests {
    private func smartConfigBody() -> Data {
        Data("""
        {
          "authorization_endpoint": "https://fhir.example.org/oauth2/authorize",
          "token_endpoint": "https://fhir.example.org/oauth2/token"
        }
        """.utf8)
    }

    @Test("connect runs discovery, exchange, and sync; reports imported count")
    func connectHappyPath() async {
        SmartConfigStubProtocol.statusCode = 200
        SmartConfigStubProtocol.body = smartConfigBody()

        let network = MockNetworkClient()
        network.requestHandler = { method, path, _ in
            #expect(method == "POST")
            switch path {
            case Endpoints.myChartConnect:
                return MyChartConnectResponse(source: "mychart", connected: true)
            case Endpoints.myChartSync:
                return MyChartSyncResponse(source: "mychart", imported: 3, skipped: 1)
            default:
                Issue.record("unexpected path \(path)")
                fatalError()
            }
        }

        let vm = MyChartConnectViewModel(
            networkClient: network,
            urlSession: SmartConfigStubProtocol.session(),
            authorize: { authURL in
                // Echo back the state the view model put in the authorize URL,
                // as a conformant provider does.
                let state = MyChartConnectViewModel.queryValue("state", from: authURL)
                return MyChartAuthorization(code: "auth-code-123", state: state)
            }
        )
        vm.fhirBaseURL = "https://fhir.example.org/r4"

        await vm.connect()

        #expect(vm.state == .connected(imported: 3))
        // Both endpoints were hit, connect before sync.
        let paths = network.requestCalls.map(\.path)
        #expect(paths == [Endpoints.myChartConnect, Endpoints.myChartSync])
    }

    @Test("state mismatch in the callback rejects the connection (CSRF guard)")
    func stateMismatchRejected() async {
        SmartConfigStubProtocol.statusCode = 200
        SmartConfigStubProtocol.body = smartConfigBody()

        let network = MockNetworkClient()
        let vm = MyChartConnectViewModel(
            networkClient: network,
            urlSession: SmartConfigStubProtocol.session(),
            // Return a state that does NOT match what the view model sent.
            authorize: { _ in MyChartAuthorization(code: "auth-code-123", state: "attacker-state") }
        )
        vm.fhirBaseURL = "https://fhir.example.org/r4"

        await vm.connect()

        if case .error = vm.state {} else {
            Issue.record("expected error state, got \(vm.state)")
        }
        // No backend exchange should have happened.
        #expect(network.requestCalls.isEmpty)
    }

    @Test("missing state in the callback is rejected")
    func missingStateRejected() async {
        SmartConfigStubProtocol.statusCode = 200
        SmartConfigStubProtocol.body = smartConfigBody()

        let network = MockNetworkClient()
        let vm = MyChartConnectViewModel(
            networkClient: network,
            urlSession: SmartConfigStubProtocol.session(),
            authorize: { _ in MyChartAuthorization(code: "auth-code-123", state: nil) }
        )
        vm.fhirBaseURL = "https://fhir.example.org/r4"

        await vm.connect()

        if case .error = vm.state {} else {
            Issue.record("expected error state, got \(vm.state)")
        }
        #expect(network.requestCalls.isEmpty)
    }

    @Test("empty FHIR URL is rejected before any network call")
    func emptyURLRejected() async {
        let network = MockNetworkClient()
        let vm = MyChartConnectViewModel(
            networkClient: network,
            urlSession: SmartConfigStubProtocol.session(),
            authorize: { _ in MyChartAuthorization(code: "x", state: nil) }
        )
        vm.fhirBaseURL = "   "

        await vm.connect()

        if case .error = vm.state {} else {
            Issue.record("expected error state, got \(vm.state)")
        }
        #expect(network.requestCalls.isEmpty)
    }

    @Test("discovery failure surfaces an error and skips exchange")
    func discoveryFailure() async {
        SmartConfigStubProtocol.statusCode = 404
        SmartConfigStubProtocol.body = Data("not found".utf8)

        let network = MockNetworkClient()
        let vm = MyChartConnectViewModel(
            networkClient: network,
            urlSession: SmartConfigStubProtocol.session(),
            authorize: { _ in MyChartAuthorization(code: "x", state: nil) }
        )
        vm.fhirBaseURL = "https://fhir.example.org/r4"

        await vm.connect()

        if case .error = vm.state {} else {
            Issue.record("expected error state, got \(vm.state)")
        }
        #expect(network.requestCalls.isEmpty)
    }

    @Test("cancelled authorization surfaces an error")
    func authorizationCancelled() async {
        SmartConfigStubProtocol.statusCode = 200
        SmartConfigStubProtocol.body = smartConfigBody()

        let network = MockNetworkClient()
        let vm = MyChartConnectViewModel(
            networkClient: network,
            urlSession: SmartConfigStubProtocol.session(),
            authorize: { _ in throw MyChartError.authorizationFailed }
        )
        vm.fhirBaseURL = "https://fhir.example.org/r4"

        await vm.connect()

        if case .error = vm.state {} else {
            Issue.record("expected error state, got \(vm.state)")
        }
        #expect(network.requestCalls.isEmpty)
    }

    @Test("authorization URL embeds PKCE challenge and required params")
    func authorizationURLParams() {
        let network = MockNetworkClient()
        let vm = MyChartConnectViewModel(networkClient: network)

        let url = vm.buildAuthorizationURL(
            authorizationEndpoint: "https://fhir.example.org/oauth2/authorize",
            fhirBaseURL: "https://fhir.example.org/r4",
            challenge: "challenge-abc",
            state: "state-xyz"
        )

        let items = URLComponents(url: url!, resolvingAgainstBaseURL: false)!.queryItems!
        func value(_ name: String) -> String? { items.first(where: { $0.name == name })?.value }

        #expect(value("response_type") == "code")
        #expect(value("client_id") == MyChartConnectViewModel.clientID)
        #expect(value("redirect_uri") == MyChartConnectViewModel.redirectURI)
        #expect(value("code_challenge") == "challenge-abc")
        #expect(value("code_challenge_method") == "S256")
        #expect(value("aud") == "https://fhir.example.org/r4")
        #expect(value("state") == "state-xyz")
    }

    @Test("queryValue reads code and state from the redirect")
    func queryValueExtraction() {
        let url = URL(string: "ownpulse://mychart-callback?code=abc123&state=xyz")!
        #expect(MyChartConnectViewModel.queryValue("code", from: url) == "abc123")
        #expect(MyChartConnectViewModel.queryValue("state", from: url) == "xyz")

        let noCode = URL(string: "ownpulse://mychart-callback?state=xyz")!
        #expect(MyChartConnectViewModel.queryValue("code", from: noCode) == nil)
    }
}
