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
}
