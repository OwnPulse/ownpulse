// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("SettingsViewModel")
@MainActor
struct SettingsViewModelTests {
    private func makeMethods() -> [AuthMethod] {
        [
            AuthMethod(
                id: "1",
                provider: "apple",
                email: "user@icloud.com",
                createdAt: Date()
            ),
            AuthMethod(
                id: "2",
                provider: "password",
                email: nil,
                createdAt: Date()
            ),
        ]
    }

    @Test("loadAuthMethods success sets isLoadingMethods and populates authMethods")
    func loadAuthMethodsSuccess() async {
        let mock = MockNetworkClient()
        let methods = makeMethods()
        mock.requestHandler = { _, _, _ in methods }

        let vm = SettingsViewModel(networkClient: mock)

        #expect(vm.isLoadingMethods == false)
        #expect(vm.authMethods.isEmpty)

        await vm.loadAuthMethods()

        #expect(vm.isLoadingMethods == false)
        #expect(vm.authMethods.count == 2)
        #expect(vm.authMethods[0].provider == "apple")
        #expect(vm.linkError == nil)
    }

    @Test("loadAuthMethods failure sets linkError")
    func loadAuthMethodsFailure() async {
        let mock = MockNetworkClient()
        mock.requestHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "internal")
        }

        let vm = SettingsViewModel(networkClient: mock)

        await vm.loadAuthMethods()

        #expect(vm.linkError == "Failed to load linked accounts")
        #expect(vm.isLoadingMethods == false)
    }

    @Test("unlinkMethod clears linkError and linkInfo then reloads")
    func unlinkMethodClearsState() async {
        let mock = MockNetworkClient()
        let methods = makeMethods()
        mock.requestHandler = { method, _, _ in
            // Both DELETE (unlink) and GET (reload) return [AuthMethod]
            return methods
        }

        let vm = SettingsViewModel(networkClient: mock)
        vm.linkError = "previous error"
        vm.linkInfo = "previous info"

        await vm.unlinkMethod("apple")

        #expect(vm.linkError == nil)
        #expect(vm.linkInfo == nil)
        // Verify it made a DELETE call and then reloaded (GET)
        #expect(mock.requestCalls.count == 2)
        #expect(mock.requestCalls[0].method == "DELETE")
        #expect(mock.requestCalls[1].method == "GET")
    }

    @Test("linkGoogle sets linkInfo not linkError")
    func linkGoogleSetsInfo() {
        let mock = MockNetworkClient()
        let vm = SettingsViewModel(networkClient: mock)

        vm.linkGoogle()

        #expect(vm.linkInfo != nil)
        #expect(vm.linkInfo!.contains("web dashboard"))
        #expect(vm.linkError == nil)
    }
}
