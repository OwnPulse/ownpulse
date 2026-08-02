// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import UserNotifications
@testable import OwnPulse

/// Test double for `UserNotificationCenterProtocol`. Tracks every added and
/// removed request so tests can assert on `NotificationManager`'s scheduling
/// behavior without touching the real, process-wide notification center.
final class MockUserNotificationCenter: UserNotificationCenterProtocol, @unchecked Sendable {
    var authorizationGranted = true
    var addError: Error?

    /// Configurable stub for `authorizationStatus()` — defaults to
    /// `.authorized` so existing scheduling tests don't need to opt in.
    /// Tests exercising the authorization-gating behavior (denied/
    /// notDetermined) set this directly.
    var stubbedAuthorizationStatus: UNAuthorizationStatus = .authorized

    private(set) var addedRequests: [UNNotificationRequest] = []
    private(set) var removedIdentifierBatches: [[String]] = []

    /// Pre-seeded pending requests, as if a previous run had scheduled them.
    var pendingRequests: [UNNotificationRequest] = []

    func requestAuthorization(options: UNAuthorizationOptions) async throws -> Bool {
        // Mirrors what a real prompt would do to the subsequent
        // authorizationStatus() read.
        stubbedAuthorizationStatus = authorizationGranted ? .authorized : .denied
        return authorizationGranted
    }

    func authorizationStatus() async -> UNAuthorizationStatus {
        stubbedAuthorizationStatus
    }

    func add(_ request: UNNotificationRequest) async throws {
        if let addError {
            throw addError
        }
        addedRequests.append(request)
        pendingRequests.append(request)
    }

    func pendingNotificationRequests() async -> [UNNotificationRequest] {
        pendingRequests
    }

    func removePendingNotificationRequests(withIdentifiers identifiers: [String]) {
        removedIdentifierBatches.append(identifiers)
        pendingRequests.removeAll { identifiers.contains($0.identifier) }
    }
}
