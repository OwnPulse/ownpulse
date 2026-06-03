// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import AuthenticationServices
import Foundation
import Observation
import os
import SwiftUI

private let logger = Logger(subsystem: "health.ownpulse.app", category: "mychart")

// MARK: - Models

/// Request body for `POST /integrations/mychart/connect`.
struct MyChartConnectRequest: Encodable, Sendable {
    let fhirBaseUrl: String
    let tokenEndpoint: String
    let code: String
    let redirectUri: String
    let codeVerifier: String

    enum CodingKeys: String, CodingKey {
        case fhirBaseUrl = "fhir_base_url"
        case tokenEndpoint = "token_endpoint"
        case code
        case redirectUri = "redirect_uri"
        case codeVerifier = "code_verifier"
    }
}

struct MyChartConnectResponse: Decodable, Sendable {
    let source: String
    let connected: Bool
}

struct MyChartSyncResponse: Decodable, Sendable {
    let source: String
    let imported: Int
    let skipped: Int
}

/// Subset of a SMART-on-FHIR `.well-known/smart-configuration` document.
struct SmartConfiguration: Decodable, Sendable {
    let authorizationEndpoint: String
    let tokenEndpoint: String

    enum CodingKeys: String, CodingKey {
        case authorizationEndpoint = "authorization_endpoint"
        case tokenEndpoint = "token_endpoint"
    }
}

/// Result of the in-app authorization step: the captured authorization code
/// and the `state` the provider echoed back (validated against the sent value
/// to prevent CSRF / code-injection).
struct MyChartAuthorization: Sendable {
    let code: String
    let state: String?
}

enum MyChartError: LocalizedError {
    case invalidBaseURL
    case discoveryFailed
    case authorizationFailed
    case missingCode
    case stateMismatch

    var errorDescription: String? {
        switch self {
        case .invalidBaseURL: "The FHIR server address is not a valid URL."
        case .discoveryFailed: "Could not read the provider's SMART configuration."
        case .authorizationFailed: "Authorization was cancelled or failed."
        case .missingCode: "The provider did not return an authorization code."
        case .stateMismatch: "Authorization response failed a security check. Please try again."
        }
    }
}

// MARK: - View Model

@Observable
@MainActor
final class MyChartConnectViewModel {
    enum State: Equatable {
        case idle
        case connecting
        case importing
        case connected(imported: Int)
        case error(String)
    }

    /// SMART public-client id registered for the OwnPulse iOS app. Public
    /// clients use PKCE rather than a secret.
    static let clientID = "ownpulse-ios"
    /// Custom-scheme redirect captured by the web auth session.
    static let redirectURI = "ownpulse://mychart-callback"
    /// FHIR scopes requested: read lab observations + reports, plus refresh.
    static let scope = "openid fhirUser launch/patient patient/Observation.read patient/DiagnosticReport.read offline_access"

    private(set) var state: State = .idle
    var fhirBaseURL: String = ""

    private let networkClient: NetworkClientProtocol
    private let urlSession: URLSession
    /// The authorization step is injected so it can be exercised in unit tests
    /// without presenting `ASWebAuthenticationSession`. In the app it defaults
    /// to the web-auth implementation below.
    private let authorize: @MainActor (URL) async throws -> MyChartAuthorization

    init(
        networkClient: NetworkClientProtocol,
        urlSession: URLSession = .shared,
        authorize: (@MainActor (URL) async throws -> MyChartAuthorization)? = nil
    ) {
        self.networkClient = networkClient
        self.urlSession = urlSession
        self.authorize = authorize ?? MyChartConnectViewModel.webAuthAuthorize
    }

    /// Full connect flow: discover SMART endpoints, run PKCE authorization,
    /// exchange the code via the backend, then trigger an initial import.
    func connect() async {
        let trimmed = fhirBaseURL.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let baseURL = URL(string: trimmed) else {
            state = .error(MyChartError.invalidBaseURL.localizedDescription)
            return
        }

        state = .connecting
        do {
            let config = try await discoverSmartConfiguration(baseURL: baseURL)
            let verifier = PKCEHelper.generateCodeVerifier()
            let challenge = PKCEHelper.codeChallenge(from: verifier)
            let sentState = UUID().uuidString

            guard let authURL = buildAuthorizationURL(
                authorizationEndpoint: config.authorizationEndpoint,
                fhirBaseURL: trimmed,
                challenge: challenge,
                state: sentState
            ) else {
                state = .error(MyChartError.discoveryFailed.localizedDescription)
                return
            }

            let authorization = try await authorize(authURL)

            // CSRF guard: the provider must echo back the exact `state` we sent.
            guard authorization.state == sentState else {
                logger.error("MyChart OAuth state mismatch")
                state = .error(MyChartError.stateMismatch.localizedDescription)
                return
            }

            let request = MyChartConnectRequest(
                fhirBaseUrl: trimmed,
                tokenEndpoint: config.tokenEndpoint,
                code: authorization.code,
                redirectUri: Self.redirectURI,
                codeVerifier: verifier
            )

            let _: MyChartConnectResponse = try await networkClient.request(
                method: "POST",
                path: Endpoints.myChartConnect,
                body: request
            )

            // Connected — now pull the first batch of labs.
            state = .importing
            let sync: MyChartSyncResponse = try await networkClient.request(
                method: "POST",
                path: Endpoints.myChartSync,
                body: nil as String?
            )
            state = .connected(imported: sync.imported)
            logger.info("MyChart connected; imported \(sync.imported) labs")
        } catch let error as MyChartError {
            state = .error(error.localizedDescription)
        } catch {
            logger.error("MyChart connect failed: \(error.localizedDescription, privacy: .public)")
            state = .error("Could not connect to MyChart. Please try again.")
        }
    }

    // MARK: - SMART discovery

    /// Fetch and parse the provider's `.well-known/smart-configuration`.
    func discoverSmartConfiguration(baseURL: URL) async throws -> SmartConfiguration {
        let wellKnown = baseURL
            .appendingPathComponent(".well-known")
            .appendingPathComponent("smart-configuration")

        var request = URLRequest(url: wellKnown)
        request.setValue("application/json", forHTTPHeaderField: "Accept")

        do {
            let (data, response) = try await urlSession.data(for: request)
            guard let http = response as? HTTPURLResponse, (200...299).contains(http.statusCode) else {
                throw MyChartError.discoveryFailed
            }
            return try JSONDecoder().decode(SmartConfiguration.self, from: data)
        } catch is MyChartError {
            throw MyChartError.discoveryFailed
        } catch {
            throw MyChartError.discoveryFailed
        }
    }

    /// Build the SMART authorization URL (authorization-code + PKCE).
    /// Exposed (internal) for unit testing.
    func buildAuthorizationURL(
        authorizationEndpoint: String,
        fhirBaseURL: String,
        challenge: String,
        state: String
    ) -> URL? {
        guard var components = URLComponents(string: authorizationEndpoint) else { return nil }
        components.queryItems = [
            URLQueryItem(name: "response_type", value: "code"),
            URLQueryItem(name: "client_id", value: Self.clientID),
            URLQueryItem(name: "redirect_uri", value: Self.redirectURI),
            URLQueryItem(name: "scope", value: Self.scope),
            URLQueryItem(name: "state", value: state),
            URLQueryItem(name: "aud", value: fhirBaseURL),
            URLQueryItem(name: "code_challenge", value: challenge),
            URLQueryItem(name: "code_challenge_method", value: "S256"),
        ]
        return components.url
    }

    /// Extract a named query parameter from a redirect URL.
    /// Exposed (internal) for unit testing.
    static func queryValue(_ name: String, from url: URL) -> String? {
        URLComponents(url: url, resolvingAgainstBaseURL: false)?
            .queryItems?
            .first(where: { $0.name == name })?
            .value
    }

    // MARK: - Web auth

    private static let presentationContext = MyChartPresentationContext()

    /// Default authorization implementation using `ASWebAuthenticationSession`.
    @MainActor
    private static func webAuthAuthorize(_ authURL: URL) async throws -> MyChartAuthorization {
        let callbackURL = try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<URL, Error>) in
            let session = ASWebAuthenticationSession(
                url: authURL,
                callback: .customScheme("ownpulse")
            ) { url, error in
                if let error {
                    continuation.resume(throwing: error)
                } else if let url {
                    continuation.resume(returning: url)
                } else {
                    continuation.resume(throwing: MyChartError.authorizationFailed)
                }
            }
            // Ephemeral session: don't share cookies with Safari for a medical
            // portal login, and don't persist the portal session on-device.
            session.prefersEphemeralWebBrowserSession = true
            session.presentationContextProvider = presentationContext
            if !session.start() {
                continuation.resume(throwing: MyChartError.authorizationFailed)
            }
        }

        guard let code = queryValue("code", from: callbackURL) else {
            throw MyChartError.missingCode
        }
        return MyChartAuthorization(code: code, state: queryValue("state", from: callbackURL))
    }
}

/// Provides a window anchor for the MyChart `ASWebAuthenticationSession`.
private final class MyChartPresentationContext: NSObject, ASWebAuthenticationPresentationContextProviding {
    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = scene.windows.first else {
            return ASPresentationAnchor()
        }
        return window
    }
}

// MARK: - View

struct MyChartConnectView: View {
    @State private var viewModel: MyChartConnectViewModel

    init(networkClient: NetworkClientProtocol) {
        _viewModel = State(initialValue: MyChartConnectViewModel(networkClient: networkClient))
    }

    /// Test/preview initializer accepting a pre-built view model.
    init(viewModel: MyChartConnectViewModel) {
        _viewModel = State(initialValue: viewModel)
    }

    var body: some View {
        Form {
            Section {
                Text("Connect your hospital or clinic's MyChart patient portal to import lab results automatically.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            Section("FHIR server address") {
                TextField("https://fhir.yourprovider.org/r4", text: $viewModel.fhirBaseURL)
                    .textContentType(.URL)
                    .keyboardType(.URL)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .accessibilityIdentifier("myChartFhirBaseURLField")
            }

            Section {
                Button(action: { Task { await viewModel.connect() } }) {
                    HStack {
                        switch viewModel.state {
                        case .connecting:
                            ProgressView()
                            Text("Connecting…")
                        case .importing:
                            ProgressView()
                            Text("Importing labs…")
                        default:
                            Text("Connect MyChart")
                        }
                    }
                }
                .disabled(isBusy)
                .accessibilityIdentifier("myChartConnectButton")
            }

            switch viewModel.state {
            case let .connected(imported):
                Section {
                    Label("Connected — imported \(imported) lab result\(imported == 1 ? "" : "s").", systemImage: "checkmark.circle.fill")
                        .foregroundStyle(.green)
                        .accessibilityIdentifier("myChartConnectedLabel")
                }
            case let .error(message):
                Section {
                    Label(message, systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(.red)
                        .accessibilityIdentifier("myChartErrorLabel")
                }
            default:
                EmptyView()
            }
        }
        .navigationTitle("MyChart")
    }

    private var isBusy: Bool {
        switch viewModel.state {
        case .connecting, .importing: true
        default: false
        }
    }
}
