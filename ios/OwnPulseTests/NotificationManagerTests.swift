// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("NotificationManager", .serialized)
@MainActor
struct NotificationManagerTests {

    @Test("registerDeviceToken sends hex-encoded token to backend")
    func registerDeviceTokenSuccess() async {
        let mock = MockNetworkClient()
        var capturedBody: RegisterPushTokenRequest?
        mock.requestNoContentHandler = { method, path, body in
            if let req = body as? RegisterPushTokenRequest {
                capturedBody = req
            }
        }

        let manager = NotificationManager(networkClient: mock)
        let tokenData = Data([0xAB, 0xCD, 0xEF, 0x01, 0x23])

        await manager.registerDeviceToken(tokenData)

        #expect(capturedBody?.deviceToken == "abcdef0123")
        #expect(capturedBody?.platform == "ios")
        #expect(mock.requestCalls.count == 1)
        #expect(mock.requestCalls[0].method == "POST")
        #expect(mock.requestCalls[0].path == Endpoints.notificationsRegister)
        #expect(manager.registrationError == nil)
    }

    @Test("registerDeviceToken sets registrationError on network failure")
    func registerDeviceTokenFailure() async {
        let mock = MockNetworkClient()
        mock.requestNoContentHandler = { _, _, _ in
            throw NetworkError.serverError(statusCode: 500, body: "internal")
        }

        let manager = NotificationManager(networkClient: mock)
        let tokenData = Data([0x01, 0x02])

        await manager.registerDeviceToken(tokenData)

        #expect(manager.registrationError == "Failed to register for notifications")
    }

    @Test("registerDeviceToken clears previous error on success")
    func registerDeviceTokenClearsError() async {
        let mock = MockNetworkClient()
        mock.requestNoContentHandler = { _, _, _ in }

        let manager = NotificationManager(networkClient: mock)
        manager.registrationError = "previous error"

        await manager.registerDeviceToken(Data([0x01]))

        #expect(manager.registrationError == nil)
    }
}
