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

    /// Snapshot of the buffered, not-yet-flushed events. Test-only inspection
    /// point — production code flushes via `flush()`.
    func bufferedEvents() -> [TelemetryEvent] {
        pendingEvents
    }

    func track(event: String, screen: String? = nil, flow: String? = nil, outcome: String? = nil) {
        guard TelemetrySettings.isEnabled else { return }

        var payload: [String: TelemetryValue] = [:]
        if let screen { payload["screen"] = .string(screen) }
        if let flow { payload["flow"] = .string(flow) }
        if let outcome { payload["outcome"] = .string(outcome) }

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

    /// Record one `api_call` event for a completed network request.
    ///
    /// Belt-and-suspenders privacy: only the allowlisted, non-identifying fields
    /// are sent (endpoint, method, status_code, latency_ms, retry_count) and the
    /// endpoint is normalized client-side so no path-segment identifiers leave
    /// the device. Never call this for the telemetry report endpoint itself —
    /// that would create a feedback loop — but `flush()` already skips it.
    func trackAPICall(
        endpoint: String,
        method: String,
        statusCode: Int,
        latencyMs: Int,
        retryCount: Int
    ) {
        guard TelemetrySettings.isEnabled else { return }

        // Never record the telemetry report call itself — it would loop.
        let normalized = FlowTracker.normalizeEndpoint(endpoint)
        guard normalized != FlowTracker.normalizeEndpoint(Endpoints.telemetryReport) else { return }

        // status_code / latency_ms / retry_count are encoded as JSON integers —
        // the backend's api_call scrubber drops string-typed numerics.
        let payload: [String: TelemetryValue] = [
            "endpoint": .string(normalized),
            "method": .string(method.uppercased()),
            "status_code": .int(statusCode),
            "latency_ms": .int(max(0, latencyMs)),
            "retry_count": .int(max(0, retryCount)),
        ]

        pendingEvents.append(TelemetryEvent(
            type: "api_call",
            deviceId: nil,
            payload: payload,
            appVersion: version,
            platform: "ios"
        ))

        if pendingEvents.count >= 20 {
            Task { await flush() }
        }
    }

    /// Collapse any non-route-word path segment to `:id`, mirroring the
    /// backend's allowlist (`normalize_endpoint`). A segment survives only if it
    /// is short (<= 24 chars) and composed solely of lowercase ASCII letters and
    /// underscores. UUIDs, emails, numbers, hex/base64 tokens, mixed-case, and
    /// hyphenated segments all become `:id`. Query strings and fragments are
    /// dropped. This errs toward over-collapsing — the privacy-safe failure mode.
    ///
    /// LEAK BOUNDARY — read before adding endpoints: this is a *shape* allowlist
    /// (see `isRouteWord`), not a known-word allowlist. It cannot distinguish a
    /// static route word from a user-controlled lowercase value, so a path that
    /// interpolates a username, slug, or substance name as a *bare* lowercase
    /// segment (e.g. `/saved-medicines/by-name/caffeine`) would transmit it
    /// verbatim. Today every dynamic path segment the app produces is a UUID or a
    /// fixed route enum, so nothing user-controlled leaks. If you add an endpoint
    /// that puts a user value in the path, you MUST keep that value out of the
    /// path (use a query parameter or request body — neither is recorded); do not
    /// rely on this normalizer to scrub it. Matches the backend's identical
    /// shape-based constraint.
    static func normalizeEndpoint(_ endpoint: String) -> String {
        let path = endpoint.split(separator: "?", maxSplits: 1, omittingEmptySubsequences: false)[0]
            .split(separator: "#", maxSplits: 1, omittingEmptySubsequences: false)[0]
        if path.isEmpty { return "unknown" }

        return path
            .split(separator: "/", omittingEmptySubsequences: false)
            .map { seg -> String in
                let s = String(seg)
                return (s.isEmpty || isRouteWord(s)) ? s : ":id"
            }
            .joined(separator: "/")
    }

    /// A *shape* test, not a dictionary check: returns true for any short
    /// all-lowercase-ASCII-letters-or-underscore segment. It therefore lets a
    /// lowercase user value through unchanged — see the LEAK BOUNDARY note on
    /// `normalizeEndpoint`. Keep user-controlled values out of URL paths.
    private static func isRouteWord(_ seg: String) -> Bool {
        guard !seg.isEmpty, seg.count <= 24 else { return false }
        return seg.allSatisfy { $0.isASCII && ($0.isLowercase && $0.isLetter || $0 == "_") }
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
