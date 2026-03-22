// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import AuthenticationServices
import SwiftUI

enum LoginMethod: Sendable {
    case apple, google, password
}

@Observable
@MainActor
final class LoginViewModel {
    var username = ""
    var password = ""
    var loadingMethod: LoginMethod?
    var errorMessage: String?

    private let authService: AuthServiceProtocol

    init(authService: AuthServiceProtocol) {
        self.authService = authService
    }

    var isLoading: Bool { loadingMethod != nil }

    func performLogin(_ method: LoginMethod) {
        guard loadingMethod == nil else { return }
        loadingMethod = method
        errorMessage = nil

        Task {
            do {
                switch method {
                case .apple:
                    try await authService.loginWithApple()
                case .google:
                    try await authService.loginWithGoogle()
                case .password:
                    try await authService.loginWithPassword(
                        username: username,
                        password: password
                    )
                    password = ""
                }
            } catch {
                errorMessage = error.localizedDescription
            }
            loadingMethod = nil
        }
    }
}

struct LoginView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var viewModel: LoginViewModel?

    var body: some View {
        ScrollView {
            VStack(spacing: 24) {
                Spacer(minLength: 48)

                VStack(spacing: 8) {
                    Text("OwnPulse")
                        .font(.largeTitle)
                        .fontWeight(.bold)

                    Text("Your health data, your control.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                }

                Spacer(minLength: 32)

                if let vm = viewModel {
                    loginButtons(vm: vm)
                    dividerRow()
                    passwordFields(vm: vm)

                    if let errorMessage = vm.errorMessage {
                        Text(errorMessage)
                            .foregroundStyle(.red)
                            .font(.caption)
                            .multilineTextAlignment(.center)
                            .accessibilityIdentifier("loginError")
                    }
                }

                Spacer(minLength: 24)
            }
            .padding(.horizontal, 24)
        }
        .onAppear {
            if viewModel == nil {
                viewModel = LoginViewModel(authService: dependencies.authService)
            }
        }
    }

    // MARK: - Private

    @ViewBuilder
    private func loginButtons(vm: LoginViewModel) -> some View {
        VStack(spacing: 12) {
            Button {
                vm.performLogin(.apple)
            } label: {
                HStack(spacing: 6) {
                    if vm.loadingMethod == .apple {
                        ProgressView()
                            .tint(.white)
                    } else {
                        Image(systemName: "apple.logo")
                    }
                    Text("Sign in with Apple")
                }
                .frame(maxWidth: .infinity)
                .frame(height: 50)
                .background(.black)
                .foregroundStyle(.white)
                .clipShape(RoundedRectangle(cornerRadius: 8))
            }
            .disabled(vm.isLoading)
            .accessibilityIdentifier("appleSignInButton")

            Button {
                vm.performLogin(.google)
            } label: {
                HStack {
                    if vm.loadingMethod == .google {
                        ProgressView()
                            .tint(.white)
                    }
                    Text("Sign in with Google")
                }
                .frame(maxWidth: .infinity)
                .frame(height: 50)
                .background(.blue)
                .foregroundStyle(.white)
                .clipShape(RoundedRectangle(cornerRadius: 8))
            }
            .disabled(vm.isLoading)
            .accessibilityIdentifier("googleSignInButton")
        }
    }

    @ViewBuilder
    private func passwordFields(vm: LoginViewModel) -> some View {
        VStack(spacing: 12) {
            TextField("Username", text: Bindable(vm).username)
                .textContentType(.username)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .textFieldStyle(.roundedBorder)
                .accessibilityIdentifier("usernameField")

            SecureField("Password", text: Bindable(vm).password)
                .textContentType(.password)
                .textFieldStyle(.roundedBorder)
                .accessibilityIdentifier("passwordField")

            Button {
                vm.performLogin(.password)
            } label: {
                Group {
                    if vm.loadingMethod == .password {
                        ProgressView()
                            .tint(.white)
                    } else {
                        Text("Sign In")
                    }
                }
                .frame(maxWidth: .infinity)
                .frame(height: 50)
                .background(.primary)
                .foregroundStyle(.background)
                .clipShape(RoundedRectangle(cornerRadius: 8))
            }
            .disabled(vm.isLoading || vm.username.isEmpty || vm.password.isEmpty)
            .accessibilityIdentifier("passwordSignInButton")
        }
    }

    @ViewBuilder
    private func dividerRow() -> some View {
        HStack {
            Rectangle()
                .frame(height: 1)
                .foregroundStyle(.quaternary)
            Text("or")
                .font(.caption)
                .foregroundStyle(.secondary)
                .padding(.horizontal, 8)
            Rectangle()
                .frame(height: 1)
                .foregroundStyle(.quaternary)
        }
    }
}
