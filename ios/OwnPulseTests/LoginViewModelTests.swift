// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("LoginViewModel", .serialized)
@MainActor
struct LoginViewModelTests {
    @Test("performLogin(.apple) calls loginWithApple on auth service")
    func loginApple() async {
        let mock = MockAuthService()
        let vm = LoginViewModel(authService: mock)

        vm.performLogin(.apple)
        // Wait for the internal Task to complete
        try? await Task.sleep(for: .milliseconds(50))

        #expect(mock.loginWithAppleCalled == true)
        #expect(mock.loginWithGoogleCalled == false)
        #expect(mock.loginWithPasswordCalled == false)
    }

    @Test("performLogin(.google) calls loginWithGoogle on auth service")
    func loginGoogle() async {
        let mock = MockAuthService()
        let vm = LoginViewModel(authService: mock)

        vm.performLogin(.google)
        try? await Task.sleep(for: .milliseconds(50))

        #expect(mock.loginWithGoogleCalled == true)
        #expect(mock.loginWithAppleCalled == false)
    }

    @Test("performLogin(.password) calls loginWithPassword with correct args")
    func loginPassword() async {
        let mock = MockAuthService()
        let vm = LoginViewModel(authService: mock)
        vm.username = "tony"
        vm.password = "secret123"

        vm.performLogin(.password)
        try? await Task.sleep(for: .milliseconds(50))

        #expect(mock.loginWithPasswordCalled == true)
        #expect(mock.loginWithPasswordArgs?.username == "tony")
        #expect(mock.loginWithPasswordArgs?.password == "secret123")
    }

    @Test("password is cleared after successful password login")
    func passwordClearedAfterLogin() async {
        let mock = MockAuthService()
        let vm = LoginViewModel(authService: mock)
        vm.username = "tony"
        vm.password = "secret123"

        vm.performLogin(.password)
        try? await Task.sleep(for: .milliseconds(50))

        #expect(vm.password == "")
    }

    @Test("double-tap guard prevents concurrent login")
    func doubleTapGuard() async {
        let mock = MockAuthService()
        let vm = LoginViewModel(authService: mock)

        // First login sets loadingMethod
        vm.performLogin(.apple)
        // Immediately try second login — should be ignored
        vm.performLogin(.google)

        try? await Task.sleep(for: .milliseconds(50))

        #expect(mock.loginWithAppleCalled == true)
        #expect(mock.loginWithGoogleCalled == false)
    }

    @Test("error message is set on login failure")
    func loginFailureSetsError() async {
        let mock = MockAuthService()
        mock.loginError = NSError(domain: "test", code: 1, userInfo: [
            NSLocalizedDescriptionKey: "Auth failed",
        ])
        let vm = LoginViewModel(authService: mock)

        vm.performLogin(.apple)
        try? await Task.sleep(for: .milliseconds(50))

        #expect(vm.errorMessage == "Auth failed")
        #expect(vm.loadingMethod == nil)
    }

    @Test("password is cleared on failed password login")
    func passwordClearedOnFailure() async {
        let mock = MockAuthService()
        mock.loginError = NSError(domain: "test", code: 1, userInfo: [
            NSLocalizedDescriptionKey: "Wrong password",
        ])
        let vm = LoginViewModel(authService: mock)
        vm.username = "tony"
        vm.password = "wrong"

        vm.performLogin(.password)
        try? await Task.sleep(for: .milliseconds(50))

        #expect(vm.password == "")
        #expect(vm.errorMessage == "Wrong password")
    }

    @Test("cancelLogin cancels running task")
    func cancelLoginCancelsTask() async {
        let mock = MockAuthService()
        let vm = LoginViewModel(authService: mock)

        vm.performLogin(.apple)
        vm.cancelLogin()

        #expect(vm.loadingMethod == nil)
    }
}
