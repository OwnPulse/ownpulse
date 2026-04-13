// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

struct MetricsResponse: Codable, Sendable {
    let sources: [MetricSourceGroup]
}

struct MetricSourceGroup: Codable, Sendable, Identifiable {
    let source: String
    let label: String
    let metrics: [MetricOptionItem]
    var id: String { "\(source).\(label)" }
}

struct MetricOptionItem: Codable, Sendable, Identifiable {
    let field: String
    let label: String
    let unit: String
    var id: String { field }
}

struct InterventionMarker: Codable, Sendable, Identifiable {
    let t: String
    let substance: String
    let dose: Double?
    let unit: String?
    let route: String?
    var id: String { "\(t).\(substance)" }

    var date: Date {
        ISO8601DateFormatter().date(from: t) ?? Date()
    }
}

extension Endpoints {
    static let exploreMetrics = "/api/v1/explore/metrics"
    static let exploreSeries = "/api/v1/explore/series"
}
