// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation

struct RegisterPushTokenRequest: Codable, Sendable {
    let deviceToken: String
    let platform: String

    enum CodingKeys: String, CodingKey {
        case deviceToken = "device_token"
        case platform
    }
}

struct NotificationPreferences: Codable, Sendable {
    let defaultNotify: Bool
    let defaultNotifyTimes: [String]
    let repeatReminders: Bool
    let repeatIntervalMinutes: Int

    enum CodingKeys: String, CodingKey {
        case defaultNotify = "default_notify"
        case defaultNotifyTimes = "default_notify_times"
        case repeatReminders = "repeat_reminders"
        case repeatIntervalMinutes = "repeat_interval_minutes"
    }
}
