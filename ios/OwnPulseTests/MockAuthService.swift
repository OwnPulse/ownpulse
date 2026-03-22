// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
@testable import OwnPulse

@MainActor
final class MockAuthService: AuthServiceProtocol {
    private(set) var isAuthenticated = false

    var loginWithGoogleCalled = false
    var loginWithAppleCalled = false
    var loginWithPasswordCalled = false
    var loginWithPasswordArgs: (username: String, password: String)?
    var logoutCalled = false
    var handleCallbackCalled = false
    var handleCallbackURL: URL?

    var loginError: Error?

    func loginWithGoogle() async throws {
        loginWithGoogleCalled = true
        if let error = loginError { throw error }
        isAuthenticated = true
    }

    func loginWithApple() async throws {
        loginWithAppleCalled = true
        if let error = loginError { throw error }
        isAuthenticated = true
    }

    func loginWithPassword(username: String, password: String) async throws {
        loginWithPasswordCalled = true
        loginWithPasswordArgs = (username, password)
        if let error = loginError { throw error }
        isAuthenticated = true
    }

    func logout() async {
        logoutCalled = true
        isAuthenticated = false
    }

    func handleCallback(url: URL) {
        handleCallbackCalled = true
        handleCallbackURL = url
    }
}
