// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
@preconcurrency import UserNotifications
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "notifications")

/// Protocol for notification management, enabling test doubles.
protocol NotificationManagerProtocol: Sendable {
    /// Request notification permission from the user. Returns whether permission was granted.
    func requestPermission() async -> Bool
    /// Register the device token with the backend.
    func registerDeviceToken(_ tokenData: Data) async
    /// Current authorization status.
    func authorizationStatus() async -> UNAuthorizationStatus
}

@Observable
@MainActor
final class NotificationManager: NotificationManagerProtocol, @unchecked Sendable {
    var isPermissionGranted = false
    var registrationError: String?

    private let networkClient: NetworkClientProtocol
    private let center: UNUserNotificationCenter

    init(
        networkClient: NetworkClientProtocol,
        notificationCenter: UNUserNotificationCenter = .current()
    ) {
        self.networkClient = networkClient
        self.center = notificationCenter
    }

    func requestPermission() async -> Bool {
        do {
            let granted = try await center.requestAuthorization(options: [.alert, .sound, .badge])
            isPermissionGranted = granted
            if granted {
                logger.info("Notification permission granted")
            } else {
                logger.info("Notification permission denied")
            }
            return granted
        } catch {
            logger.error("Failed to request notification permission: \(error.localizedDescription, privacy: .public)")
            registrationError = "Failed to request notification permission"
            return false
        }
    }

    func registerDeviceToken(_ tokenData: Data) async {
        let tokenString = tokenData.map { String(format: "%02x", $0) }.joined()
        let request = RegisterPushTokenRequest(
            deviceToken: tokenString,
            platform: "ios"
        )

        do {
            try await networkClient.requestNoContent(
                method: "POST",
                path: Endpoints.notificationsRegister,
                body: request
            )
            logger.info("Device token registered with backend")
            registrationError = nil
        } catch {
            logger.error("Failed to register device token: \(error.localizedDescription, privacy: .public)")
            registrationError = "Failed to register for notifications"
        }
    }

    func authorizationStatus() async -> UNAuthorizationStatus {
        let settings = await center.notificationSettings()
        return settings.authorizationStatus
    }
}
