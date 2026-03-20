// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct LoginView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var isLoggingIn = false
    @State private var errorMessage: String?

    var body: some View {
        VStack(spacing: 24) {
            Spacer()

            Text("OwnPulse")
                .font(.largeTitle)
                .fontWeight(.bold)

            Text("Your health data, your control.")
                .font(.subheadline)
                .foregroundStyle(.secondary)

            Spacer()

            Button {
                Task {
                    isLoggingIn = true
                    errorMessage = nil
                    do {
                        try await dependencies.authService.login()
                    } catch {
                        errorMessage = error.localizedDescription
                    }
                    isLoggingIn = false
                }
            } label: {
                HStack {
                    if isLoggingIn {
                        ProgressView()
                            .tint(.white)
                    }
                    Text("Sign in with Google")
                }
                .frame(maxWidth: .infinity)
                .padding()
                .background(.blue)
                .foregroundStyle(.white)
                .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            .disabled(isLoggingIn)
            .accessibilityIdentifier("loginButton")

            if let errorMessage {
                Text(errorMessage)
                    .foregroundStyle(.red)
                    .font(.caption)
                    .accessibilityIdentifier("loginError")
            }

            Spacer()
        }
        .padding()
    }
}
