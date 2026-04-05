// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "feature-flags")

struct AppConfigResponse: Codable, Sendable {
    let featureFlags: [String: Bool]
    let ios: IosConfig

    enum CodingKeys: String, CodingKey {
        case featureFlags = "feature_flags"
        case ios
    }

    struct IosConfig: Codable, Sendable {
        let minSupportedVersion: String?
        let forceUpgradeBelow: String?

        enum CodingKeys: String, CodingKey {
            case minSupportedVersion = "min_supported_version"
            case forceUpgradeBelow = "force_upgrade_below"
        }
    }
}

@MainActor
protocol FeatureFlagServiceProtocol: Sendable {
    func isEnabled(_ key: String) -> Bool
    func fetch() async
}

@Observable
@MainActor
final class FeatureFlagService: FeatureFlagServiceProtocol, @unchecked Sendable {
    private(set) var flags: [String: Bool] = [:]
    private(set) var isLoaded = false
    private(set) var forceUpgradeBelow: String?
    private(set) var minSupportedVersion: String?

    private let networkClient: NetworkClientProtocol
    private var lastFetchTime: Date?
    private static let cacheKey = "cached_feature_flags"
    private static let minFetchInterval: TimeInterval = 60 // 1 minute

    init(networkClient: NetworkClientProtocol) {
        self.networkClient = networkClient
        // Load cached flags for instant offline access
        if let cached = UserDefaults.standard.dictionary(forKey: Self.cacheKey) as? [String: Bool] {
            self.flags = cached
        }
    }

    func isEnabled(_ key: String) -> Bool {
        flags[key] ?? false
    }

    func fetch() async {
        // Rate limit: at most once per minute
        if let last = lastFetchTime, Date().timeIntervalSince(last) < Self.minFetchInterval {
            return
        }

        do {
            let config: AppConfigResponse = try await networkClient.request(
                method: "GET",
                path: "/api/v1/config",
                body: nil
            )
            flags = config.featureFlags
            forceUpgradeBelow = config.ios.forceUpgradeBelow
            minSupportedVersion = config.ios.minSupportedVersion
            isLoaded = true
            lastFetchTime = Date()

            // Cache for offline use
            UserDefaults.standard.set(config.featureFlags, forKey: Self.cacheKey)
            logger.info("Feature flags loaded: \(config.featureFlags.count) flags")
        } catch {
            logger.error("Failed to fetch feature flags: \(error.localizedDescription, privacy: .public)")
            // Keep using cached values -- don't clear flags on failure
            isLoaded = true // Mark as loaded even on failure so UI doesn't block
        }
    }

    /// Returns true if the current app version is below the force-upgrade threshold.
    var requiresForceUpgrade: Bool {
        guard let threshold = forceUpgradeBelow,
              let current = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String else {
            return false
        }
        return current.compare(threshold, options: .numeric) == .orderedAscending
    }
}
