// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("NotificationModels")
struct NotificationModelsTests {

    @Test("RegisterPushTokenRequest encodes with snake_case keys")
    func registerPushTokenRequestEncoding() throws {
        let request = RegisterPushTokenRequest(
            deviceToken: "abc123",
            platform: "ios"
        )
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        let data = try encoder.encode(request)
        let json = try #require(String(data: data, encoding: .utf8))

        #expect(json.contains("\"device_token\""))
        #expect(json.contains("\"abc123\""))
        #expect(json.contains("\"platform\""))
        #expect(json.contains("\"ios\""))
    }

    @Test("RegisterPushTokenRequest decodes from snake_case JSON")
    func registerPushTokenRequestDecoding() throws {
        let json = """
        {"device_token":"hex123","platform":"ios"}
        """
        let data = Data(json.utf8)
        let decoded = try JSONDecoder().decode(RegisterPushTokenRequest.self, from: data)

        #expect(decoded.deviceToken == "hex123")
        #expect(decoded.platform == "ios")
    }

    @Test("NotificationPreferences decodes from snake_case JSON")
    func notificationPreferencesDecoding() throws {
        let json = """
        {
            "default_notify": true,
            "default_notify_times": ["08:00", "20:00"],
            "repeat_reminders": false,
            "repeat_interval_minutes": 30
        }
        """
        let data = Data(json.utf8)
        let decoded = try JSONDecoder().decode(NotificationPreferences.self, from: data)

        #expect(decoded.defaultNotify == true)
        #expect(decoded.defaultNotifyTimes == ["08:00", "20:00"])
        #expect(decoded.repeatReminders == false)
        #expect(decoded.repeatIntervalMinutes == 30)
    }

    @Test("NotificationPreferences encodes with snake_case keys")
    func notificationPreferencesEncoding() throws {
        let prefs = NotificationPreferences(
            defaultNotify: true,
            defaultNotifyTimes: ["09:00"],
            repeatReminders: true,
            repeatIntervalMinutes: 15
        )
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        let data = try encoder.encode(prefs)
        let json = try #require(String(data: data, encoding: .utf8))

        #expect(json.contains("\"default_notify\""))
        #expect(json.contains("\"default_notify_times\""))
        #expect(json.contains("\"repeat_reminders\""))
        #expect(json.contains("\"repeat_interval_minutes\""))
    }
}
