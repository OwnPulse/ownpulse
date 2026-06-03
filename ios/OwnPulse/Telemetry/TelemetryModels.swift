// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

struct TelemetryReport: Codable, Sendable {
    let events: [TelemetryEvent]
}

/// A single telemetry payload value. Most events carry strings, but `api_call`
/// events carry integers for `status_code`, `latency_ms`, and `retry_count` —
/// the backend's `api_call` scrubber only accepts JSON integers for those keys
/// (a string `"200"` is dropped), so they must be encoded as numbers, not
/// quoted strings.
enum TelemetryValue: Codable, Sendable, Equatable, ExpressibleByStringLiteral {
    case string(String)
    case int(Int)

    init(stringLiteral value: String) {
        self = .string(value)
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let s): try container.encode(s)
        case .int(let i): try container.encode(i)
        }
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let i = try? container.decode(Int.self) {
            self = .int(i)
        } else {
            self = .string(try container.decode(String.self))
        }
    }
}

struct TelemetryEvent: Codable, Sendable {
    let type: String
    let deviceId: String?
    let payload: [String: TelemetryValue]
    let appVersion: String?
    /// Originating platform. Sent as `"ios"` for `api_call` events; `nil` for
    /// other event types (the backend defaults the column to `"ios"`).
    let platform: String?

    init(
        type: String,
        deviceId: String?,
        payload: [String: TelemetryValue],
        appVersion: String?,
        platform: String? = nil
    ) {
        self.type = type
        self.deviceId = deviceId
        self.payload = payload
        self.appVersion = appVersion
        self.platform = platform
    }

    enum CodingKeys: String, CodingKey {
        case type
        case deviceId = "device_id"
        case payload
        case appVersion = "app_version"
        case platform
    }
}

struct TelemetryResponse: Codable, Sendable {
    let accepted: Int
    let rejected: Int
}

extension Endpoints {
    static let telemetryReport = "/api/v1/telemetry/report"
}
