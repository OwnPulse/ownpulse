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

    private(set) var addedRequests: [UNNotificationRequest] = []
    private(set) var removedIdentifierBatches: [[String]] = []

    /// Pre-seeded pending requests, as if a previous run had scheduled them.
    var pendingRequests: [UNNotificationRequest] = []

    func requestAuthorization(options: UNAuthorizationOptions) async throws -> Bool {
        authorizationGranted
    }

    func notificationSettings() async -> UNNotificationSettings {
        // `UNNotificationSettings` has no public initializer, so it can't be
        // faked here. None of the dose-reminder tests in this suite need
        // it — `NotificationManager.authorizationStatus()` is covered
        // separately in `NotificationManagerTests` against the real
        // `UNUserNotificationCenter`.
        preconditionFailure("MockUserNotificationCenter.notificationSettings() is unsupported — UNNotificationSettings has no test initializer")
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
