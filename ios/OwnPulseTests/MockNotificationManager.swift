// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import UserNotifications
@testable import OwnPulse

@MainActor
final class MockNotificationManager: NotificationManagerProtocol, @unchecked Sendable {
    var permissionGranted = true
    var currentStatus: UNAuthorizationStatus = .notDetermined
    var registeredTokens: [Data] = []
    var requestPermissionCallCount = 0

    func requestPermission() async -> Bool {
        requestPermissionCallCount += 1
        return permissionGranted
    }

    func registerDeviceToken(_ tokenData: Data) async {
        registeredTokens.append(tokenData)
    }

    func authorizationStatus() async -> UNAuthorizationStatus {
        currentStatus
    }
}
