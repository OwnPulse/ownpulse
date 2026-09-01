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
/// center. `authorizationStatus()` isn't one of `UNUserNotificationCenter`'s
/// native methods — it only exposes authorization via `notificationSettings()`,
/// and `UNNotificationSettings` has no public initializer a mock could
/// return. The extension below bridges the two, so the mock can just hand
/// back the enum directly.
protocol UserNotificationCenterProtocol: Sendable {
    func requestAuthorization(options: UNAuthorizationOptions) async throws -> Bool
    func authorizationStatus() async -> UNAuthorizationStatus
    func add(_ request: UNNotificationRequest) async throws
    func pendingNotificationRequests() async -> [UNNotificationRequest]
    func removePendingNotificationRequests(withIdentifiers identifiers: [String])
}

// `UNUserNotificationCenter.current()` is documented as safe to call from
// any thread, which is what this conformance actually asserts — the
// `@preconcurrency import` above doesn't grant this on its own.
extension UNUserNotificationCenter: @unchecked Sendable {}

extension UNUserNotificationCenter: UserNotificationCenterProtocol {
    func authorizationStatus() async -> UNAuthorizationStatus {
        await notificationSettings().authorizationStatus
    }
}

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
    ///
    /// Gated on notification authorization: prompts once if the status is
    /// still `.notDetermined` and there's something to schedule, and skips
    /// scheduling quietly (single log line) when denied, rather than making
    /// (and logging) up to 64 doomed `add()` calls.
    func scheduleDoseReminders(runs: [DoseReminderRun], now: Date) async
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
        await center.authorizationStatus()
    }

    // MARK: - Dose Reminders

    func scheduleDoseReminders(runs: [DoseReminderRun], now: Date = Date()) async {
        // The 64-pending cap is an app-wide budget, not a dose-reminder-only
        // one — reserve room for whatever non-dose notifications are already
        // pending rather than assuming the whole budget is ours to spend.
        let pending = await center.pendingNotificationRequests()
        let existingDoseIds = Set(pending.map(\.identifier).filter { $0.hasPrefix(doseReminderIdentifierPrefix) })
        let nonDoseCount = pending.count - existingDoseIds.count
        let budget = max(0, DoseReminderScheduler.maxPendingNotifications - nonDoseCount)

        let (specs, truncatedCount) = DoseReminderScheduler.computeSpecs(runs: runs, now: now, maxPending: budget)
        if truncatedCount > 0 {
            logger.warning(
                "Dropped \(truncatedCount, privacy: .public) dose reminder(s) past the 64-pending-notification system cap"
            )
        }

        var status = await center.authorizationStatus()
        if status == .notDetermined, !specs.isEmpty {
            _ = await requestPermission()
            status = await center.authorizationStatus()
        }
        guard status == .authorized || status == .provisional else {
            if status == .denied {
                logger.notice("Notification permission denied — skipping dose reminder scheduling")
            }
            return
        }

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

    func clearAllDoseReminders() async {
        let pending = await center.pendingNotificationRequests()
        let ids = pending.map(\.identifier).filter { $0.hasPrefix(doseReminderIdentifierPrefix) }
        guard !ids.isEmpty else { return }
        center.removePendingNotificationRequests(withIdentifiers: ids)
    }
}
