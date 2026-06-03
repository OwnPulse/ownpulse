// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import XCTest

/// Accessibility audit gate.
///
/// Runs Apple's built-in `performAccessibilityAudit()` against the screens the
/// app presents on a clean launch. The audit inspects the live view hierarchy
/// for issues VoiceOver / Dynamic Type / contrast / hit-region users would hit:
/// missing element descriptions, clipped text, insufficient contrast,
/// hit-targets smaller than 44x44pt, and elements that trait as buttons but
/// carry no label.
///
/// The audit *fails the test* when it finds an issue, so this is a real CI gate,
/// not a smoke test.
///
/// ## Scope
///
/// On a clean launch (no Keychain session) the app shows `LoginView`. That is
/// the only screen reachable deterministically without a backend or a signed-in
/// session, so it is what we audit here. The authenticated tab screens
/// (Dashboard / Protocols / Log / Explore / Settings) require live auth state
/// and network data; auditing them needs a launch-time auth/network seam that
/// does not yet exist in the app and would mean modifying app views — out of
/// scope for this test. When that seam lands, add per-screen audit methods here
/// that navigate via the accessibility identifiers on `MainTabView`
/// ("mainTabView") and call `app.performAccessibilityAudit()` on each tab.
///
/// This test class is a `final class` deliberately: XCUITest discovers tests via
/// the ObjC runtime and Swift Testing does not support UI tests, so XCTest is
/// the correct (and only) framework here despite the repo's Swift-Testing default.
final class AccessibilityAuditTests: XCTestCase {

    override func setUp() {
        super.setUp()
        // A failed audit should report every issue it finds, not stop at the
        // first one — that makes the failure log actionable in one pass.
        continueAfterFailure = true
    }

    /// Audits the unauthenticated launch screen (`LoginView`).
    ///
    /// We do not pass an exclusion set: every audit category should apply to the
    /// login screen. If a category ever produces an unavoidable false positive
    /// (e.g. a system-rendered control we cannot fix), narrow it by passing a
    /// reduced `XCUIAccessibilityAuditType` option set to
    /// `performAccessibilityAudit(for:)` and document the specific reason inline
    /// — never blanket-disable categories.
    @MainActor
    func testLoginScreenPassesAccessibilityAudit() throws {
        let app = XCUIApplication()
        app.launch()

        // Confirm we are on the login screen before auditing, so the audit is
        // never run against an unexpected hierarchy (which would make a pass
        // meaningless). Element lookup is by accessibility identifier per the
        // repo convention — never by visible text.
        XCTAssertTrue(
            app.buttons["appleSignInButton"].waitForExistence(timeout: 10),
            "Expected LoginView (appleSignInButton) on clean launch before running the audit"
        )

        try app.performAccessibilityAudit()
    }
}
