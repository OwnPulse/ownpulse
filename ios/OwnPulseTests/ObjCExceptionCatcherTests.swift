// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("ObjCExceptionCatcher")
struct ObjCExceptionCatcherTests {
    // MARK: - Bridging an Objective-C exception into a Swift error
    //
    // HealthKit's requestAuthorization raises NSException when a type in
    // `toShare` is disallowed. Swift can't catch Objective-C exceptions, so
    // without this bridge the process crashes with SIGABRT. These tests pin
    // the bridge behaviour so it doesn't silently regress.

    // Swift imports `+tryBlock:error:` as a throwing method (standard NSError
    // out-parameter pattern), so these tests use try/catch rather than a
    // Bool return.

    @Test("tryBlock does not throw when the block completes normally")
    func noExceptionDoesNotThrow() throws {
        var ran = false
        try ObjCExceptionCatcher.`try` { ran = true }
        #expect(ran == true)
    }

    @Test("tryBlock converts NSException into a thrown NSError")
    func nsExceptionBecomesThrownNSError() {
        do {
            try ObjCExceptionCatcher.`try` {
                NSException(
                    name: .invalidArgumentException,
                    reason: "simulated failure",
                    userInfo: nil
                ).raise()
            }
            Issue.record("Expected tryBlock to throw")
        } catch {
            let nsError = error as NSError
            #expect(nsError.domain == "OwnPulseObjCException")
            #expect(nsError.localizedDescription == "simulated failure")
            #expect((nsError.userInfo["ExceptionName"] as? NSExceptionName) == .invalidArgumentException)
        }
    }
}
