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
        var receivedToken: Data?

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
}
