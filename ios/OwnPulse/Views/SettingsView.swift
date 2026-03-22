// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import AuthenticationServices
import SwiftUI
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "settings")

@Observable
@MainActor
private final class SettingsViewModel {
    var authMethods: [AuthMethod] = []
    var isLoadingMethods = false
    var linkError: String?

    private let networkClient: NetworkClientProtocol
    private let authService: AuthServiceProtocol

    init(networkClient: NetworkClientProtocol, authService: AuthServiceProtocol) {
        self.networkClient = networkClient
        self.authService = authService
    }

    func loadAuthMethods() async {
        isLoadingMethods = true
        do {
            authMethods = try await networkClient.request(
                method: "GET",
                path: Endpoints.authMethods,
                body: Optional<String>.none
            )
        } catch {
            logger.error("Failed to load auth methods: \(error.localizedDescription, privacy: .public)")
        }
        isLoadingMethods = false
    }

    func unlinkMethod(_ provider: String) async {
        linkError = nil
        do {
            let _: [AuthMethod] = try await networkClient.request(
                method: "DELETE",
                path: "\(Endpoints.authLink)/\(provider)",
                body: Optional<String>.none
            )
            await loadAuthMethods()
        } catch {
            logger.error("Failed to unlink \(provider, privacy: .public): \(error.localizedDescription, privacy: .public)")
            linkError = "Failed to unlink \(provider.capitalized): \(error.localizedDescription)"
        }
    }

    func linkApple() async {
        linkError = nil
        do {
            let provider = ASAuthorizationAppleIDProvider()
            let request = provider.createRequest()
            request.requestedScopes = [.email]

            let authorization = try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<ASAuthorization, Error>) in
                let controller = ASAuthorizationController(authorizationRequests: [request])
                let delegate = LinkAppleDelegate(continuation: continuation)
                objc_setAssociatedObject(
                    controller,
                    &linkAppleDelegateKey,
                    delegate,
                    .OBJC_ASSOCIATION_RETAIN_NONATOMIC
                )
                controller.delegate = delegate
                controller.presentationContextProvider = linkApplePresentationContext
                controller.performRequests()
            }

            guard let credential = authorization.credential as? ASAuthorizationAppleIDCredential,
                  let idTokenData = credential.identityToken,
                  let idToken = String(data: idTokenData, encoding: .utf8) else {
                throw AuthError.invalidCallback
            }

            let body = LinkAuthRequest(provider: "apple", idToken: idToken, password: nil)
            let _: [AuthMethod] = try await networkClient.request(
                method: "POST",
                path: Endpoints.authLink,
                body: body
            )
            await loadAuthMethods()
        } catch {
            logger.error("Failed to link Apple: \(error.localizedDescription, privacy: .public)")
            linkError = "Failed to link Apple account: \(error.localizedDescription)"
        }
    }

    func linkGoogle() async {
        linkError = nil
        // Google linking opens the same OAuth flow as login. The backend links the
        // authenticated Google account to the current user session.
        // This shares the same ASWebAuthenticationSession flow as the login path.
        // For MVP, redirect users to the web dashboard to link Google accounts,
        // since the link endpoint requires an id_token from a completed OAuth flow.
        linkError = "To link a Google account, use the web dashboard."
    }
}

// MARK: - Apple Link Helpers

private final class LinkAppleDelegate: NSObject, ASAuthorizationControllerDelegate, Sendable {
    private let continuation: CheckedContinuation<ASAuthorization, Error>

    init(continuation: CheckedContinuation<ASAuthorization, Error>) {
        self.continuation = continuation
    }

    func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithAuthorization authorization: ASAuthorization
    ) {
        continuation.resume(returning: authorization)
    }

    func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithError error: Error
    ) {
        continuation.resume(throwing: error)
    }
}

private class LinkApplePresentationContext: NSObject,
    ASAuthorizationControllerPresentationContextProviding
{
    func presentationAnchor(for controller: ASAuthorizationController) -> ASPresentationAnchor {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = scene.windows.first else {
            return ASPresentationAnchor()
        }
        return window
    }
}

private var linkAppleDelegateKey: UInt8 = 0
private let linkApplePresentationContext = LinkApplePresentationContext()

// MARK: - SettingsView

struct SettingsView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var showLogoutConfirmation = false
    @State private var hkAuthorized = false
    @State private var viewModel: SettingsViewModel?

    var body: some View {
        List {
            Section("HealthKit") {
                HStack {
                    Text("Authorization")
                    Spacer()
                    Text(hkAuthorized ? "Granted" : "Not Authorized")
                        .foregroundStyle(.secondary)
                }
                .accessibilityIdentifier("hkAuthStatus")

                if !hkAuthorized {
                    Button("Request Access") {
                        Task {
                            try? await dependencies.healthKitProvider.requestAuthorization()
                            hkAuthorized = dependencies.healthKitProvider.isAuthorized()
                        }
                    }
                    .accessibilityIdentifier("requestHKAccessButton")
                }
            }

            if let vm = viewModel {
                linkedAccountsSection(vm: vm)
            }

            Section("Dashboard") {
                Link("Open Web Dashboard", destination: AppConfig.webDashboardURL)
                    .accessibilityIdentifier("openDashboardLink")
            }

            Section {
                Button("Sign Out", role: .destructive) {
                    showLogoutConfirmation = true
                }
                .accessibilityIdentifier("logoutButton")
            }
        }
        .navigationTitle("Settings")
        .onAppear {
            hkAuthorized = dependencies.healthKitProvider.isAuthorized()
            if viewModel == nil {
                viewModel = SettingsViewModel(
                    networkClient: dependencies.networkClient,
                    authService: dependencies.authService
                )
            }
            Task { await viewModel?.loadAuthMethods() }
        }
        .confirmationDialog("Sign out?", isPresented: $showLogoutConfirmation) {
            Button("Sign Out", role: .destructive) {
                Task {
                    await dependencies.authService.logout()
                }
            }
        }
    }

    @ViewBuilder
    private func linkedAccountsSection(vm: SettingsViewModel) -> some View {
        Section("Linked Accounts") {
            if vm.isLoadingMethods {
                ProgressView()
                    .accessibilityIdentifier("linkedAccountsLoading")
            } else {
                ForEach(vm.authMethods) { method in
                    HStack {
                        Image(systemName: iconForProvider(method.provider))
                            .frame(width: 24)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(method.provider.capitalized)
                            if let email = method.email {
                                Text(email)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        Spacer()
                        if vm.authMethods.count > 1 {
                            Button("Unlink", role: .destructive) {
                                Task { await vm.unlinkMethod(method.provider) }
                            }
                            .accessibilityIdentifier("unlink-\(method.provider)")
                        }
                    }
                }

                if !vm.authMethods.contains(where: { $0.provider == "apple" }) {
                    Button("Link Apple Account") {
                        Task { await vm.linkApple() }
                    }
                    .accessibilityIdentifier("linkAppleButton")
                }

                if !vm.authMethods.contains(where: { $0.provider == "google" }) {
                    Button("Link Google Account") {
                        Task { await vm.linkGoogle() }
                    }
                    .accessibilityIdentifier("linkGoogleButton")
                }
            }

            if let error = vm.linkError {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .accessibilityIdentifier("linkError")
            }
        }
    }

    private func iconForProvider(_ provider: String) -> String {
        switch provider.lowercased() {
        case "apple":
            return "applelogo"
        case "google":
            return "globe"
        case "password":
            return "key.fill"
        default:
            return "person.circle"
        }
    }
}
