// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import UIKit
import UserNotifications
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "notifications")

/// Handles APNs device token registration and notification tap actions.
/// Retained by the App as a @State property for the lifetime of the app.
final class NotificationDelegate: NSObject, UIApplicationDelegate, @unchecked Sendable {
    /// Callback invoked when APNs delivers a device token.
    var onDeviceToken: (@Sendable (Data) -> Void)?

    /// Callback invoked when user taps a notification. The String is the
    /// notification category identifier (e.g., "dose_reminder").
    var onNotificationTap: (@Sendable (String) -> Void)?

    // MARK: - UIApplicationDelegate

    func application(
        _ application: UIApplication,
        didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data
    ) {
        logger.info("Received APNs device token")
        onDeviceToken?(deviceToken)
    }

    func application(
        _ application: UIApplication,
        didFailToRegisterForRemoteNotificationsWithError error: Error
    ) {
        logger.error(
            "Failed to register for remote notifications: \(error.localizedDescription, privacy: .public)"
        )
    }
}

// MARK: - UNUserNotificationCenterDelegate

@preconcurrency extension NotificationDelegate: UNUserNotificationCenterDelegate {
    /// Called when a notification is delivered while the app is in the foreground.
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        [.banner, .sound]
    }

    /// Called when the user taps a notification.
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse
    ) async {
        let categoryIdentifier = response.notification.request.content.categoryIdentifier
        logger.info("Notification tapped: category=\(categoryIdentifier, privacy: .public)")
        onNotificationTap?(categoryIdentifier)
    }
}
