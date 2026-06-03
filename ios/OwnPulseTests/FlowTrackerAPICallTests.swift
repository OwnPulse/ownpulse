// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing

@testable import OwnPulse

/// Tests for the `api_call` telemetry path on `FlowTracker`:
/// endpoint normalization/scrubbing, the opt-in gate, and the guarantee that
/// no request/response bodies are ever carried in an event payload.
///
/// `TelemetrySettings.isEnabled` is backed by `UserDefaults`, which is global
/// process state, so each test that toggles it restores the prior value.
struct FlowTrackerAPICallTests {

    // MARK: - Endpoint normalization / scrubbing

    @Test("UUID path segments collapse to :id")
    func uuidSegmentScrubbed() {
        #expect(
            FlowTracker.normalizeEndpoint(
                "/api/v1/protocols/550e8400-e29b-41d4-a716-446655440000/runs"
            ) == "/api/v1/protocols/:id/runs"
        )
        // Dashless UUID is still not a route word.
        #expect(
            FlowTracker.normalizeEndpoint(
                "/records/550e8400e29b41d4a716446655440000"
            ) == "/records/:id"
        )
    }

    @Test("Numeric path segments collapse to :id")
    func numericSegmentScrubbed() {
        #expect(
            FlowTracker.normalizeEndpoint("/api/v1/users/12345/profile")
                == "/api/v1/users/:id/profile"
        )
    }

    @Test("Email path segments collapse to :id")
    func emailSegmentScrubbed() {
        #expect(
            FlowTracker.normalizeEndpoint("/users/alice@example.com/profile")
                == "/users/:id/profile"
        )
    }

    @Test("Query strings and fragments are dropped")
    func queryAndFragmentDropped() {
        #expect(
            FlowTracker.normalizeEndpoint("/records/abc123?token=secret&x=1")
                == "/records/:id"
        )
        #expect(
            FlowTracker.normalizeEndpoint("/api/v1/health_records#frag")
                == "/api/v1/health_records"
        )
    }

    @Test("Plain lowercase route words survive")
    func routeWordsSurvive() {
        #expect(
            FlowTracker.normalizeEndpoint("/api/v1/health_records")
                == "/api/v1/health_records"
        )
    }

    @Test("Mixed-case and hyphenated segments collapse to :id")
    func mixedCaseAndHyphenScrubbed() {
        // Hyphenated segment is not all-lowercase-letters-or-underscore.
        #expect(FlowTracker.normalizeEndpoint("/source-preferences") == "/:id")
        // Uppercase collapses too.
        #expect(FlowTracker.normalizeEndpoint("/Users") == "/:id")
    }

    @Test("Over-long lowercase segment collapses to :id")
    func overLongSegmentScrubbed() {
        let longSeg = String(repeating: "a", count: 25)
        #expect(FlowTracker.normalizeEndpoint("/\(longSeg)") == "/:id")
    }

    // MARK: - Opt-in gate

    @Test("No event is buffered when telemetry is disabled")
    func disabledRecordsNothing() async {
        let previous = TelemetrySettings.isEnabled
        defer { TelemetrySettings.isEnabled = previous }
        TelemetrySettings.isEnabled = false

        let tracker = FlowTracker()
        await tracker.trackAPICall(
            endpoint: "/api/v1/health-records",
            method: "GET",
            statusCode: 200,
            latencyMs: 12,
            retryCount: 0
        )

        let buffered = await tracker.bufferedEvents()
        #expect(buffered.isEmpty)
    }

    @Test("An event is buffered when telemetry is enabled")
    func enabledRecordsEvent() async {
        let previous = TelemetrySettings.isEnabled
        defer { TelemetrySettings.isEnabled = previous }
        TelemetrySettings.isEnabled = true

        let tracker = FlowTracker()
        await tracker.trackAPICall(
            endpoint: "/api/v1/users/12345/profile",
            method: "get",
            statusCode: 200,
            latencyMs: 42,
            retryCount: 1
        )

        let buffered = await tracker.bufferedEvents()
        #expect(buffered.count == 1)
        let event = try! #require(buffered.first)
        #expect(event.type == "api_call")
        #expect(event.platform == "ios")
        #expect(event.deviceId == nil)
        #expect(event.payload["endpoint"] == .string("/api/v1/users/:id/profile"))
        #expect(event.payload["method"] == .string("GET"))
        #expect(event.payload["status_code"] == .int(200))
        #expect(event.payload["latency_ms"] == .int(42))
        #expect(event.payload["retry_count"] == .int(1))
    }

    @Test("The telemetry report endpoint is never recorded (no feedback loop)")
    func telemetryEndpointSkipped() async {
        let previous = TelemetrySettings.isEnabled
        defer { TelemetrySettings.isEnabled = previous }
        TelemetrySettings.isEnabled = true

        let tracker = FlowTracker()
        await tracker.trackAPICall(
            endpoint: Endpoints.telemetryReport,
            method: "POST",
            statusCode: 200,
            latencyMs: 5,
            retryCount: 0
        )

        let buffered = await tracker.bufferedEvents()
        #expect(buffered.isEmpty)
    }

    /// Mirrors the sequence `NetworkClient.performRequest` emits on a 401 that is
    /// refreshed and retried successfully: the initial 401 attempt (retry_count 0)
    /// is recorded *before* the refresh, then the successful retry (retry_count 1).
    /// This locks in the contract that a refresh which later throws still leaves
    /// one recorded event for the request, and that a successful refresh produces
    /// two distinguishable events rather than zero.
    @Test("401-refresh-retry sequence records both the 401 and the retry")
    func authRefreshRetrySequenceRecorded() async {
        let previous = TelemetrySettings.isEnabled
        defer { TelemetrySettings.isEnabled = previous }
        TelemetrySettings.isEnabled = true

        let tracker = FlowTracker()
        // Initial attempt: 401, recorded before refresh is attempted.
        await tracker.trackAPICall(
            endpoint: "/api/v1/health-records",
            method: "GET",
            statusCode: 401,
            latencyMs: 8,
            retryCount: 0
        )
        // Refresh succeeded; retry succeeds with retry_count 1.
        await tracker.trackAPICall(
            endpoint: "/api/v1/health-records",
            method: "GET",
            statusCode: 200,
            latencyMs: 11,
            retryCount: 1
        )

        let buffered = await tracker.bufferedEvents()
        #expect(buffered.count == 2)
        #expect(buffered[0].payload["status_code"] == .int(401))
        #expect(buffered[0].payload["retry_count"] == .int(0))
        #expect(buffered[1].payload["status_code"] == .int(200))
        #expect(buffered[1].payload["retry_count"] == .int(1))
    }

    // MARK: - Bodies are never included

    @Test("api_call payload carries only the allowlisted fields — no body keys")
    func payloadHasOnlyAllowlistedFields() async {
        let previous = TelemetrySettings.isEnabled
        defer { TelemetrySettings.isEnabled = previous }
        TelemetrySettings.isEnabled = true

        let tracker = FlowTracker()
        await tracker.trackAPICall(
            endpoint: "/api/v1/health-records",
            method: "POST",
            statusCode: 201,
            latencyMs: 30,
            retryCount: 0
        )

        let buffered = await tracker.bufferedEvents()
        let event = try! #require(buffered.first)

        let allowed: Set<String> = ["endpoint", "method", "status_code", "latency_ms", "retry_count"]
        #expect(Set(event.payload.keys) == allowed)
    }

    @Test("Encoded api_call event JSON contains no body/request/response keys")
    func encodedEventHasNoBodyKeys() async throws {
        let previous = TelemetrySettings.isEnabled
        defer { TelemetrySettings.isEnabled = previous }
        TelemetrySettings.isEnabled = true

        let tracker = FlowTracker()
        await tracker.trackAPICall(
            endpoint: "/api/v1/labs/bulk",
            method: "POST",
            statusCode: 200,
            latencyMs: 7,
            retryCount: 0
        )
        let buffered = await tracker.bufferedEvents()
        let report = TelemetryReport(events: buffered)

        let data = try JSONEncoder().encode(report)
        let json = try #require(String(data: data, encoding: .utf8)).lowercased()

        // None of these substrings should appear: only allowlisted scalar fields ship.
        for forbidden in ["\"body\"", "\"request\"", "\"response\"", "\"value\"", "\"numeric\""] {
            #expect(!json.contains(forbidden), "encoded event leaked \(forbidden)")
        }
    }

    @Test("Numeric api_call fields encode as JSON integers, not quoted strings")
    func numericFieldsEncodeAsIntegers() async throws {
        let previous = TelemetrySettings.isEnabled
        defer { TelemetrySettings.isEnabled = previous }
        TelemetrySettings.isEnabled = true

        let tracker = FlowTracker()
        await tracker.trackAPICall(
            endpoint: "/api/v1/health-records",
            method: "GET",
            statusCode: 200,
            latencyMs: 42,
            retryCount: 1
        )
        let buffered = await tracker.bufferedEvents()
        let report = TelemetryReport(events: buffered)

        let data = try JSONEncoder().encode(report)
        let json = try #require(String(data: data, encoding: .utf8))

        // The backend's api_call scrubber drops string-typed numerics, so these
        // must be bare integers. A quoted form would be silently discarded.
        #expect(json.contains("\"status_code\":200"))
        #expect(json.contains("\"latency_ms\":42"))
        #expect(json.contains("\"retry_count\":1"))
        #expect(!json.contains("\"status_code\":\"200\""))
    }

    @Test("Negative latency and retry count are clamped to zero")
    func negativeValuesClamped() async {
        let previous = TelemetrySettings.isEnabled
        defer { TelemetrySettings.isEnabled = previous }
        TelemetrySettings.isEnabled = true

        let tracker = FlowTracker()
        await tracker.trackAPICall(
            endpoint: "/api/v1/health-records",
            method: "GET",
            statusCode: 500,
            latencyMs: -10,
            retryCount: -3
        )
        let buffered = await tracker.bufferedEvents()
        let event = try! #require(buffered.first)
        #expect(event.payload["latency_ms"] == .int(0))
        #expect(event.payload["retry_count"] == .int(0))
    }
}
