// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Testing
@testable import OwnPulse

@Suite("AppConfig")
struct AppConfigTests {
    @Test("versionString returns version and build in expected format")
    func versionStringFormat() {
        let version = AppConfig.versionString
        // Format should be "X.Y.Z (N)" — at minimum non-empty with parenthesized build
        #expect(version.contains("("))
        #expect(version.contains(")"))
        #expect(!version.hasPrefix("?"))
    }
}
