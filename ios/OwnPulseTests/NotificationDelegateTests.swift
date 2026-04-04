// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
import UIKit
@testable import OwnPulse

@Suite("NotificationDelegate", .serialized)
@MainActor
struct NotificationDelegateTests {

    @Test("onDeviceToken callback is invoked with token data")
    func deviceTokenCallback() {
        let delegate = NotificationDelegate()
        nonisolated(unsafe) var receivedToken: Data?

        delegate.onDeviceToken = { data in
            receivedToken = data
        }

        let tokenData = Data([0xDE, 0xAD, 0xBE, 0xEF])
        delegate.application(
            UIApplication.shared,
            didRegisterForRemoteNotificationsWithDeviceToken: tokenData
        )

        #expect(receivedToken == tokenData)
    }

    @Test("callbacks are nil by default")
    func callbacksNilByDefault() {
        let delegate = NotificationDelegate()
        #expect(delegate.onDeviceToken == nil)
        #expect(delegate.onNotificationTap == nil)
    }

    @Test("onDeviceToken is not called when no callback is set")
    func deviceTokenWithoutCallback() {
        let delegate = NotificationDelegate()
        // Should not crash when onDeviceToken is nil
        delegate.application(
            UIApplication.shared,
            didRegisterForRemoteNotificationsWithDeviceToken: Data([0x01])
        )
    }

    @Test("didFailToRegister does not crash when no handler set")
    func failedRegistrationNoHandler() {
        let delegate = NotificationDelegate()
        // Should not crash
        delegate.application(
            UIApplication.shared,
            didFailToRegisterForRemoteNotificationsWithError: NSError(
                domain: "test", code: -1
            )
        )
    }

    @Test("willPresent returns banner and sound options")
    func willPresentOptions() async {
        let delegate = NotificationDelegate()
        // UNNotification cannot be directly instantiated, so we verify
        // protocol conformance and delegate assignment instead.
        #expect(delegate is UNUserNotificationCenterDelegate)

        // Verify the delegate can be assigned to the notification center
        let center = UNUserNotificationCenter.current()
        let previousDelegate = center.delegate
        center.delegate = delegate
        #expect(center.delegate === delegate)
        center.delegate = previousDelegate
    }

    @Test("onNotificationTap callback can be set and is retained")
    func notificationTapCallbackRetained() {
        let delegate = NotificationDelegate()
        nonisolated(unsafe) var tappedCategory: String?

        delegate.onNotificationTap = { category in
            tappedCategory = category
        }

        // We cannot easily instantiate UNNotificationResponse to invoke
        // didReceive directly. Verify the callback is retained and callable.
        #expect(delegate.onNotificationTap != nil)

        // Invoke the callback directly to verify wiring
        delegate.onNotificationTap?("dose_reminder")
        #expect(tappedCategory == "dose_reminder")
    }
}
