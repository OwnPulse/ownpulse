// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("FeatureFlagService")
@MainActor
struct FeatureFlagServiceTests {
    private func makeService(handler: @escaping (String, String, (any Encodable & Sendable)?) throws -> Any) -> FeatureFlagService {
        let mock = MockNetworkClient()
        mock.requestHandler = handler
        return FeatureFlagService(networkClient: mock)
    }

    private func configResponse(
        flags: [String: Bool] = [:],
        minSupportedVersion: String? = nil,
        forceUpgradeBelow: String? = nil
    ) -> AppConfigResponse {
        AppConfigResponse(
            featureFlags: flags,
            ios: .init(
                minSupportedVersion: minSupportedVersion,
                forceUpgradeBelow: forceUpgradeBelow
            )
        )
    }

    @Test("isEnabled returns false for unknown keys")
    func unknownKeyReturnsFalse() {
        let mock = MockNetworkClient()
        let service = FeatureFlagService(networkClient: mock)
        #expect(service.isEnabled("nonexistent") == false)
    }

    @Test("isEnabled returns cached value after successful fetch")
    func fetchPopulatesFlags() async {
        let service = makeService { _, _, _ in
            self.configResponse(flags: ["dashboard_v2": true, "beta_export": false])
        }

        await service.fetch()

        #expect(service.isEnabled("dashboard_v2") == true)
        #expect(service.isEnabled("beta_export") == false)
        #expect(service.isLoaded == true)
    }

    @Test("fetch failure marks isLoaded true without clearing flags")
    func fetchFailureKeepsCached() async {
        let service = makeService { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "Internal Server Error")
        }

        await service.fetch()

        #expect(service.isLoaded == true)
        #expect(service.isEnabled("anything") == false)
    }

    @Test("requiresForceUpgrade returns false when no threshold set")
    func noForceUpgradeWhenNoThreshold() {
        let mock = MockNetworkClient()
        let service = FeatureFlagService(networkClient: mock)
        #expect(service.requiresForceUpgrade == false)
    }

    @Test("requiresForceUpgrade returns true when version below threshold")
    func forceUpgradeBelowThreshold() async {
        let service = makeService { _, _, _ in
            self.configResponse(forceUpgradeBelow: "99.0.0")
        }

        await service.fetch()

        // Current app version (1.0.0 in tests) is below 99.0.0
        #expect(service.requiresForceUpgrade == true)
    }

    @Test("requiresForceUpgrade returns false when version above threshold")
    func noForceUpgradeAboveThreshold() async {
        let service = makeService { _, _, _ in
            self.configResponse(forceUpgradeBelow: "0.0.1")
        }

        await service.fetch()

        // Current app version is above 0.0.1
        #expect(service.requiresForceUpgrade == false)
    }

    @Test("rate limiting skips second fetch within 60 seconds")
    func rateLimitSkipsSecondFetch() async {
        var fetchCount = 0
        let service = makeService { _, _, _ in
            fetchCount += 1
            return self.configResponse(flags: ["flag_\(fetchCount)": true])
        }

        await service.fetch()
        #expect(fetchCount == 1)
        #expect(service.isEnabled("flag_1") == true)

        // Second fetch within 60s should be skipped
        await service.fetch()
        #expect(fetchCount == 1)
        // Flags should still be from first fetch
        #expect(service.isEnabled("flag_1") == true)
        #expect(service.isEnabled("flag_2") == false)
    }
}
