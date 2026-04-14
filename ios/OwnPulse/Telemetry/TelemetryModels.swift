// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

struct TelemetryReport: Codable, Sendable {
    let events: [TelemetryEvent]
}

struct TelemetryEvent: Codable, Sendable {
    let type: String
    let deviceId: String?
    let payload: [String: String]
    let appVersion: String?

    enum CodingKeys: String, CodingKey {
        case type
        case deviceId = "device_id"
        case payload
        case appVersion = "app_version"
    }
}

struct TelemetryResponse: Codable, Sendable {
    let accepted: Int
    let rejected: Int
}

extension Endpoints {
    static let telemetryReport = "/api/v1/telemetry/report"
}
