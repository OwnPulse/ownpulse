// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "flow-tracker")

actor FlowTracker {
    static let shared = FlowTracker()

    private var pendingEvents: [TelemetryEvent] = []
    private var networkClient: NetworkClientProtocol?
    private let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String

    func configure(networkClient: NetworkClientProtocol) {
        self.networkClient = networkClient
    }

    func track(event: String, screen: String? = nil, flow: String? = nil, outcome: String? = nil) {
        guard TelemetrySettings.isEnabled else { return }

        var payload: [String: String] = [:]
        if let screen { payload["screen"] = screen }
        if let flow { payload["flow"] = flow }
        if let outcome { payload["outcome"] = outcome }

        pendingEvents.append(TelemetryEvent(
            type: event,
            deviceId: nil,
            payload: payload,
            appVersion: version
        ))

        if pendingEvents.count >= 20 {
            Task { await flush() }
        }
    }

    func flush() async {
        guard !pendingEvents.isEmpty, let client = networkClient else { return }

        let events = pendingEvents
        pendingEvents = []

        do {
            let report = TelemetryReport(events: events)
            let _: TelemetryResponse = try await client.request(
                method: "POST",
                path: Endpoints.telemetryReport,
                body: report
            )
            logger.info("Flushed \(events.count) flow event(s)")
        } catch {
            if pendingEvents.count + events.count <= 100 {
                pendingEvents.insert(contentsOf: events, at: 0)
            }
            logger.error("Failed to flush flow events: \(error.localizedDescription, privacy: .public)")
        }
    }

    func screenViewed(_ screen: String) async {
        await track(event: "screen", screen: screen)
    }

    func flowCompleted(_ flow: String) async {
        await track(event: "flow", flow: flow, outcome: "completed")
    }

    func flowError(_ flow: String, reason: String) async {
        await track(event: "flow", flow: flow, outcome: "error_\(reason)")
    }

    func flowAbandoned(_ flow: String) async {
        await track(event: "flow", flow: flow, outcome: "abandoned")
    }
}
