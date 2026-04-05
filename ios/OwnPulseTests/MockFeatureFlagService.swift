// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
@testable import OwnPulse

@MainActor
final class MockFeatureFlagService: FeatureFlagServiceProtocol, @unchecked Sendable {
    var enabledFlags: Set<String> = []
    private(set) var fetchCallCount = 0

    func isEnabled(_ key: String) -> Bool {
        enabledFlags.contains(key)
    }

    func fetch() async {
        fetchCallCount += 1
    }
}
