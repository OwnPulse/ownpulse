// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

enum AppConfig {
    static var apiBaseURL: URL {
        #if DEBUG
        // Debug builds: check for developer override, fall back to localhost
        if let override = UserDefaults.standard.string(forKey: "api_base_url_override"),
           let url = URL(string: override) {
            return url
        }
        return URL(string: "http://localhost:8080")!
        #else
        // Release builds: always hit production
        return URL(string: "https://app.ownpulse.health")!
        #endif
    }

    static var webDashboardURL: URL {
        apiBaseURL
    }

    static var versionString: String {
        let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "?"
        let build = Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "?"
        return "\(version) (\(build))"
    }

    /// The running app's build identifier — used to stamp offline-queue
    /// entries abandoned due to an undecodable payload, so a later app
    /// version (which may have changed the payload shape back, or fixed
    /// the decoder) automatically gets a fresh chance to drain them.
    static var buildNumber: String {
        Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? "unknown"
    }

    static var gitSHA: String? {
        let sha = Bundle.main.infoDictionary?["OPGitSHA"] as? String
        // Build setting substitution leaves "$(GIT_SHA)" when unset
        return (sha?.hasPrefix("$(") == true) ? nil : sha
    }

    static var buildRef: String? {
        let ref = Bundle.main.infoDictionary?["OPBuildRef"] as? String
        return (ref?.hasPrefix("$(") == true) ? nil : ref
    }
}
