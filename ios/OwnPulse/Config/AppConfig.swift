// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

enum AppConfig {
    static var apiBaseURL: URL {
        guard let urlString = Bundle.main.infoDictionary?["API_BASE_URL"] as? String,
              let url = URL(string: urlString) else {
            fatalError("API_BASE_URL not configured")
        }
        return url
    }

    static var googleClientID: String {
        guard let clientID = Bundle.main.infoDictionary?["GOOGLE_CLIENT_ID"] as? String,
              !clientID.isEmpty else {
            fatalError("GOOGLE_CLIENT_ID not configured")
        }
        return clientID
    }

    static var webDashboardURL: URL {
        guard let urlString = Bundle.main.infoDictionary?["WEB_DASHBOARD_URL"] as? String,
              let url = URL(string: urlString) else {
            fatalError("WEB_DASHBOARD_URL not configured")
        }
        return url
    }

    static var googleRedirectURI: String {
        "\(apiBaseURL)/api/v1/auth/google/callback"
    }
}
