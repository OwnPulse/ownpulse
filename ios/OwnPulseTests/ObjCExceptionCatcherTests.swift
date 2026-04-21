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

    @Test("tryBlock returns true when the block does not raise")
    func noExceptionReturnsTrue() {
        var ran = false
        var err: NSError?
        let ok = ObjCExceptionCatcher.tryBlock({ ran = true }, error: &err)
        #expect(ok == true)
        #expect(ran == true)
        #expect(err == nil)
    }

    @Test("tryBlock converts NSException into NSError")
    func nsExceptionBecomesNSError() {
        var err: NSError?
        let ok = ObjCExceptionCatcher.tryBlock({
            NSException(
                name: .invalidArgumentException,
                reason: "simulated failure",
                userInfo: nil
            ).raise()
        }, error: &err)

        #expect(ok == false)
        #expect(err != nil)
        #expect(err?.localizedDescription == "simulated failure")
        #expect((err?.userInfo["ExceptionName"] as? NSExceptionName) == .invalidArgumentException)
    }

    @Test("tryBlock without an error pointer still suppresses the exception")
    func tolerant_of_nil_error_pointer() {
        // Passing nil for `error` must not crash — the catch still suppresses
        // the exception. Matches the contract the header advertises.
        let ok = ObjCExceptionCatcher.tryBlock({
            NSException(
                name: .genericException,
                reason: "no error pointer",
                userInfo: nil
            ).raise()
        }, error: nil)

        #expect(ok == false)
    }
}
