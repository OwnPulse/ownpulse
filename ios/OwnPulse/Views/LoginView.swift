// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import AuthenticationServices
import SwiftUI

struct LoginView: View {
    @Environment(AppDependencies.self) private var dependencies

    @State private var username = ""
    @State private var password = ""
    @State private var isLoading = false
    @State private var errorMessage: String?

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

                VStack(spacing: 12) {
                    SignInWithAppleButton(.signIn) { request in
                        request.requestedScopes = [.email]
                    } onCompletion: { _ in
                        // Completion handled inside AuthService.loginWithApple()
                        // which performs the full flow including the backend call.
                        // This closure fires only for the native button interaction;
                        // we trigger the full flow via the task below.
                    }
                    .signInWithAppleButtonStyle(.black)
                    .frame(height: 50)
                    .accessibilityIdentifier("appleSignInButton")
                    .onTapGesture {
                        performLogin(.apple)
                    }

                    Button {
                        performLogin(.google)
                    } label: {
                        HStack {
                            if isLoading {
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
                    .disabled(isLoading)
                    .accessibilityIdentifier("googleSignInButton")
                }

                dividerRow()

                VStack(spacing: 12) {
                    TextField("Username", text: $username)
                        .textContentType(.username)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                        .textFieldStyle(.roundedBorder)
                        .accessibilityIdentifier("usernameField")

                    SecureField("Password", text: $password)
                        .textContentType(.password)
                        .textFieldStyle(.roundedBorder)
                        .accessibilityIdentifier("passwordField")

                    Button {
                        performLogin(.password)
                    } label: {
                        Group {
                            if isLoading {
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
                    .disabled(isLoading || username.isEmpty || password.isEmpty)
                    .accessibilityIdentifier("passwordSignInButton")
                }

                if let errorMessage {
                    Text(errorMessage)
                        .foregroundStyle(.red)
                        .font(.caption)
                        .multilineTextAlignment(.center)
                        .accessibilityIdentifier("loginError")
                }

                Spacer(minLength: 24)
            }
            .padding(.horizontal, 24)
        }
    }

    // MARK: - Private

    private enum LoginMethod {
        case apple, google, password
    }

    private func performLogin(_ method: LoginMethod) {
        guard !isLoading else { return }
        isLoading = true
        errorMessage = nil

        Task {
            do {
                switch method {
                case .apple:
                    try await dependencies.authService.loginWithApple()
                case .google:
                    try await dependencies.authService.loginWithGoogle()
                case .password:
                    try await dependencies.authService.loginWithPassword(
                        username: username,
                        password: password
                    )
                }
            } catch {
                errorMessage = error.localizedDescription
            }
            isLoading = false
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
