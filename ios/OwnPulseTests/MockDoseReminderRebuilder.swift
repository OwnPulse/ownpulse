// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
@testable import OwnPulse

/// Test double for `DoseReminderRebuilding`, used by view-model tests that
/// only care *whether* a rebuild was triggered, not the network/notification
/// plumbing behind it (that's covered by `DoseReminderCoordinatorTests` and
/// `NotificationManagerDoseReminderTests`).
@MainActor
final class MockDoseReminderRebuilder: DoseReminderRebuilding, @unchecked Sendable {
    private(set) var rebuildCallCount = 0
    private(set) var clearAllCallCount = 0

    func rebuildReminders() async {
        rebuildCallCount += 1
    }

    func clearAll() async {
        clearAllCallCount += 1
    }
}
