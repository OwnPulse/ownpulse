// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

/// Guards the "Log" verb + terracotta primary-CTA parity web has held since
/// #310 (`feat(web): unify entry-form terminology on "Log" verb`). No
/// ViewInspector in this codebase to assert rendered button text/tint
/// directly, so this scans the view source for the exact literals instead —
/// cheap enough to catch an accidental revert of either.
@Suite("Log form terminology parity")
struct LogTerminologyTests {
    /// `#filePath` for this test file lives in `ios/OwnPulseTests/`; the
    /// views under test live in the sibling `ios/OwnPulse/Views/Log/`.
    private static func logViewsSource(_ fileName: String) throws -> String {
        let testFile = URL(fileURLWithPath: #filePath)
        let iosDir = testFile.deletingLastPathComponent().deletingLastPathComponent()
        let sourceURL = iosDir
            .appendingPathComponent("OwnPulse")
            .appendingPathComponent("Views")
            .appendingPathComponent("Log")
            .appendingPathComponent(fileName)
        return try String(contentsOf: sourceURL, encoding: .utf8)
    }

    @Test("CheckinForm submit button uses the Log verb, not Save")
    func checkinFormUsesLogVerb() throws {
        let source = try Self.logViewsSource("CheckinForm.swift")
        #expect(source.contains("\"Log Check-in\""))
        #expect(!source.contains("\"Save Check-in\""))
    }

    @Test("InterventionForm submit button is the terracotta primary CTA, not teal")
    func interventionFormUsesTerracottaCTA() throws {
        let source = try Self.logViewsSource("InterventionForm.swift")
        #expect(source.contains("\"Log Intervention\""))
        // Only the submit button's `.background(...)` should reference teal
        // (or, after this fix, terracotta) — assert the fixed color, and
        // that the submit button block no longer backgrounds on teal.
        #expect(source.contains(".background(OPColor.terracotta)"))
    }
}
