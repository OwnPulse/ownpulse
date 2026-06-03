// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("AppDependencies — widget deep-link routing")
@MainActor
struct AppDependenciesDeepLinkTests {
    private func make() -> AppDependencies {
        let network = MockNetworkClient()
        network.requestHandler = { _, _, _ in [] as [AuthMethod] }
        return AppDependencies(
            keychainService: MockKeychainService(),
            networkClient: network,
            healthKitProvider: MockHealthKitProvider()
        )
    }

    @Test("log deep link with form=checkin selects the Log tab and pre-selects check-in")
    func checkinDeepLink() {
        let deps = make()
        let handled = deps.handleDeepLink(URL(string: "ownpulse://log?form=checkin")!)
        #expect(handled == true)
        #expect(deps.selectedTab == AppDependencies.logTabIndex)
        #expect(deps.pendingLogForm == .checkin)
    }

    @Test("log deep link with form=intervention pre-selects intervention")
    func interventionDeepLink() {
        let deps = make()
        let handled = deps.handleDeepLink(URL(string: "ownpulse://log?form=intervention")!)
        #expect(handled == true)
        #expect(deps.pendingLogForm == .intervention)
    }

    @Test("log deep link with form=observation pre-selects observation")
    func observationDeepLink() {
        let deps = make()
        let handled = deps.handleDeepLink(URL(string: "ownpulse://log?form=observation")!)
        #expect(handled == true)
        #expect(deps.pendingLogForm == .observation)
    }

    @Test("log deep link with no form defaults to check-in")
    func noFormDefaultsToCheckin() {
        let deps = make()
        let handled = deps.handleDeepLink(URL(string: "ownpulse://log")!)
        #expect(handled == true)
        #expect(deps.pendingLogForm == .checkin)
    }

    @Test("log deep link with unknown form falls back to check-in")
    func unknownFormFallsBack() {
        let deps = make()
        let handled = deps.handleDeepLink(URL(string: "ownpulse://log?form=bogus")!)
        #expect(handled == true)
        #expect(deps.pendingLogForm == .checkin)
    }

    @Test("auth callback URL is NOT handled as a deep link (falls through)")
    func authUrlFallsThrough() {
        let deps = make()
        let handled = deps.handleDeepLink(
            URL(string: "ownpulse://auth#token=jwt&refresh_token=refresh")!
        )
        #expect(handled == false)
        #expect(deps.pendingLogForm == nil)
        #expect(deps.selectedTab == 0)
    }

    @Test("foreign scheme is NOT handled")
    func foreignSchemeNotHandled() {
        let deps = make()
        let handled = deps.handleDeepLink(URL(string: "https://example.com/log")!)
        #expect(handled == false)
        #expect(deps.pendingLogForm == nil)
    }
}
