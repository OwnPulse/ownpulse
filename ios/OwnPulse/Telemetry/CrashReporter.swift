// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import MetricKit
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "crash-reporter")

@MainActor
final class CrashReporter: NSObject, MXMetricManagerSubscriber {
    private let networkClient: NetworkClientProtocol

    init(networkClient: NetworkClientProtocol) {
        self.networkClient = networkClient
        super.init()
    }

    nonisolated func didReceive(_ payloads: [MXDiagnosticPayload]) {
        guard TelemetrySettings.isEnabled else { return }

        var events: [TelemetryEvent] = []
        let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String

        for payload in payloads {
            if let crashes = payload.crashDiagnostics {
                for crash in crashes {
                    var crashPayload: [String: String] = [:]

                    if let signal = crash.signal {
                        crashPayload["signal"] = "\(signal)"
                    }
                    if let exceptionType = crash.exceptionType {
                        crashPayload["exception_type"] = "\(exceptionType)"
                    }
                    if let reason = crash.terminationReason {
                        crashPayload["termination_reason"] = reason
                    }
                    let stackData = crash.callStackTree.jsonRepresentation()
                    if let stackStr = String(data: stackData, encoding: .utf8) {
                        crashPayload["call_stack_tree"] = stackStr
                    }

                    events.append(TelemetryEvent(
                        type: "crash",
                        deviceId: TelemetrySettings.deviceId,
                        payload: crashPayload,
                        appVersion: version
                    ))
                }
            }
        }

        guard !events.isEmpty else { return }

        Task { @MainActor in
            do {
                let report = TelemetryReport(events: events)
                let _: TelemetryResponse = try await networkClient.request(
                    method: "POST",
                    path: Endpoints.telemetryReport,
                    body: report
                )
                logger.info("Sent \(events.count) crash report(s)")
            } catch {
                logger.error("Failed to send crash reports: \(error.localizedDescription, privacy: .public)")
            }
        }
    }

}
