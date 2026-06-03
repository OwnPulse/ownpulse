// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing

@testable import OwnPulse

/// Verifies that every `Endpoints.*` value used by the app resolves to a
/// well-formed URL path. A missing or malformed endpoint would otherwise only
/// surface at runtime when a view tries to use it. This guards against the
/// class of regression where a `ViewModel` references an `Endpoints` member
/// that no longer exists (or never did).
struct EndpointsTests {
    /// Every static string endpoint and every computed endpoint (with a sample
    /// id) the app references. Adding a new `Endpoints` member without listing
    /// it here is fine; removing one the app uses is caught by the compiler.
    private static let allPaths: [String] = [
        // Auth
        Endpoints.authAppleCallback,
        Endpoints.authGoogleLogin,
        Endpoints.authGoogleCallback,
        Endpoints.authLogin,
        Endpoints.authLink,
        Endpoints.authMethods,
        Endpoints.authRefresh,
        // HealthKit
        Endpoints.healthKitSync,
        Endpoints.healthKitWriteQueue,
        Endpoints.healthKitConfirm,
        // Admin
        Endpoints.adminUsers,
        Endpoints.adminInvites,
        // Misc
        Endpoints.healthRecords,
        Endpoints.labsBulk,
        Endpoints.notificationsRegister,
        Endpoints.notificationPreferences,
        Endpoints.savedMedicines,
        // C8: MyChart / SMART-on-FHIR
        Endpoints.myChartConnect,
        Endpoints.myChartSync,
        // Protocols
        Endpoints.protocols,
        Endpoints.activeRuns,
        Endpoints.protocolDetail("proto-1"),
        Endpoints.protocolRuns("proto-1"),
        Endpoints.protocolLogDose("proto-1"),
        Endpoints.protocolSkipDose("proto-1"),
        Endpoints.runLogDose("run-1"),
        Endpoints.runSkipDose("run-1"),
    ]

    @Test func allEndpointsAreValidApiPaths() {
        for path in Self.allPaths {
            #expect(path.hasPrefix("/api/v1/"), "endpoint must be an /api/v1 path: \(path)")
            #expect(!path.contains(" "), "endpoint must not contain spaces: \(path)")
        }
    }

    @Test func allEndpointsResolveToValidURLs() {
        let base = AppConfig.apiBaseURL
        for path in Self.allPaths {
            let url = base.appendingPathComponent(path)
            #expect(url.absoluteString.hasPrefix(base.absoluteString),
                    "endpoint did not resolve under base URL: \(path)")
            #expect(url.path.contains("/api/v1/"),
                    "resolved URL is missing the API path: \(path)")
        }
    }

    // MARK: - Protocols

    @Test func protocolDetailEmbedsId() {
        #expect(Endpoints.protocolDetail("abc") == "/api/v1/protocols/abc")
    }

    @Test func protocolRunsEmbedsId() {
        #expect(Endpoints.protocolRuns("abc") == "/api/v1/protocols/abc/runs")
    }

    @Test func activeRunsMatchesBackendPath() {
        #expect(Endpoints.activeRuns == "/api/v1/protocols/runs/active")
    }

    @Test func protocolLogDoseEmbedsId() {
        #expect(Endpoints.protocolLogDose("abc") == "/api/v1/protocols/abc/doses/log")
    }

    @Test func protocolSkipDoseEmbedsId() {
        #expect(Endpoints.protocolSkipDose("abc") == "/api/v1/protocols/abc/doses/skip")
    }

    @Test func runLogDoseEmbedsRunId() {
        #expect(Endpoints.runLogDose("r1") == "/api/v1/protocols/runs/r1/doses/log")
    }

    @Test func runSkipDoseEmbedsRunId() {
        #expect(Endpoints.runSkipDose("r1") == "/api/v1/protocols/runs/r1/doses/skip")
    }

    @Test func protocolsListMatchesBackendPath() {
        #expect(Endpoints.protocols == "/api/v1/protocols")
    }
}
