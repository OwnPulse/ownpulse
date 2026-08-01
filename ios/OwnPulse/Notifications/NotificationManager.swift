// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
@preconcurrency import UserNotifications
import os

private let logger = Logger(subsystem: "health.ownpulse.app", category: "notifications")

/// Prefix shared by every identifier `DoseReminderScheduler` produces.
/// `NotificationManager` uses it to distinguish dose reminders it owns from
/// any other pending local notification, so rebuilds/cancellations never
/// touch requests scheduled elsewhere.
private let doseReminderIdentifierPrefix = "dose-"

/// Thin abstraction over `UNUserNotificationCenter` so scheduling logic can
/// be unit tested without touching the real, process-wide notification
/// center. `UNUserNotificationCenter` already implements every method below
/// with matching signatures, so it conforms with no extra code (see the
/// extension at the bottom of this file).
protocol UserNotificationCenterProtocol {
    func requestAuthorization(options: UNAuthorizationOptions) async throws -> Bool
    func notificationSettings() async -> UNNotificationSettings
    func add(_ request: UNNotificationRequest) async throws
    func pendingNotificationRequests() async -> [UNNotificationRequest]
    func removePendingNotificationRequests(withIdentifiers identifiers: [String])
}

extension UNUserNotificationCenter: UserNotificationCenterProtocol {}

/// Protocol for notification management, enabling test doubles.
protocol NotificationManagerProtocol: Sendable {
    /// Request notification permission from the user. Returns whether permission was granted.
    func requestPermission() async -> Bool
    /// Register the device token with the backend.
    func registerDeviceToken(_ tokenData: Data) async
    /// Current authorization status.
    func authorizationStatus() async -> UNAuthorizationStatus

    /// Rebuilds every pending local dose-reminder notification from scratch:
    /// computes the desired set via `DoseReminderScheduler`, removes any
    /// previously-scheduled dose reminder that's no longer in that set (a
    /// run that was paused/completed/deleted simply won't appear), and adds
    /// the rest. Safe to call repeatedly — deterministic identifiers mean a
    /// re-schedule replaces rather than duplicates.
    func scheduleDoseReminders(runs: [DoseReminderRun], now: Date) async
    /// Cancels every pending reminder for a single run (e.g. on pause/complete/delete).
    func clearDoseReminders(runId: String) async
    /// Cancels every pending dose reminder this app has scheduled.
    func clearAllDoseReminders() async
}

extension NotificationManagerProtocol {
    /// Convenience overload defaulting `now` to the current time. Protocol
    /// requirements can't carry default argument values, so callers going
    /// through the protocol (rather than the concrete `NotificationManager`)
    /// get this instead.
    func scheduleDoseReminders(runs: [DoseReminderRun]) async {
        await scheduleDoseReminders(runs: runs, now: Date())
    }
}

@Observable
@MainActor
final class NotificationManager: NotificationManagerProtocol, @unchecked Sendable {
    var isPermissionGranted = false
    var registrationError: String?

    private let networkClient: NetworkClientProtocol
    private let center: any UserNotificationCenterProtocol

    init(
        networkClient: NetworkClientProtocol,
        notificationCenter: any UserNotificationCenterProtocol = UNUserNotificationCenter.current()
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

    // MARK: - Dose Reminders

    func scheduleDoseReminders(runs: [DoseReminderRun], now: Date = Date()) async {
        let (specs, truncatedCount) = DoseReminderScheduler.computeSpecs(runs: runs, now: now)
        if truncatedCount > 0 {
            logger.warning(
                "Dropped \(truncatedCount, privacy: .public) dose reminder(s) past the 64-pending-notification system cap"
            )
        }

        let pending = await center.pendingNotificationRequests()
        let existingDoseIds = Set(pending.map(\.identifier).filter { $0.hasPrefix(doseReminderIdentifierPrefix) })
        let newIds = Set(specs.map(\.identifier))
        let staleIds = existingDoseIds.subtracting(newIds)
        if !staleIds.isEmpty {
            center.removePendingNotificationRequests(withIdentifiers: Array(staleIds))
        }

        for spec in specs {
            let content = UNMutableNotificationContent()
            content.title = spec.title
            content.body = spec.body
            content.sound = .default
            content.categoryIdentifier = "dose_reminder"

            let comps = Calendar.current.dateComponents(
                [.year, .month, .day, .hour, .minute],
                from: spec.fireDate
            )
            let trigger = UNCalendarNotificationTrigger(dateMatching: comps, repeats: false)
            let request = UNNotificationRequest(identifier: spec.identifier, content: content, trigger: trigger)

            do {
                try await center.add(request)
            } catch {
                logger.error(
                    "Failed to schedule dose reminder \(spec.identifier, privacy: .public): \(error.localizedDescription, privacy: .public)"
                )
            }
        }
    }

    func clearDoseReminders(runId: String) async {
        let pending = await center.pendingNotificationRequests()
        let ids = pending.map(\.identifier).filter { $0.hasPrefix("\(doseReminderIdentifierPrefix)\(runId)-") }
        guard !ids.isEmpty else { return }
        center.removePendingNotificationRequests(withIdentifiers: ids)
    }

    func clearAllDoseReminders() async {
        let pending = await center.pendingNotificationRequests()
        let ids = pending.map(\.identifier).filter { $0.hasPrefix(doseReminderIdentifierPrefix) }
        guard !ids.isEmpty else { return }
        center.removePendingNotificationRequests(withIdentifiers: ids)
    }
}
