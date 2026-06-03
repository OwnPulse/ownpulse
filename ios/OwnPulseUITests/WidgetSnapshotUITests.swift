// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import XCTest

// Verified green via the iOS CI test job (unit suites + these XCUITests).

/// Drives the DEBUG-only `WidgetSnapshotHarness` (launched via the
/// `-WidgetSnapshotHarness` argument) and asserts that each widget view
/// renders for every supported family:
///   - TodayCheckinWidget: accessoryCircular + accessoryRectangular
///   - HeroMetricWidget:    accessoryRectangular + systemSmall
///   - QuickLogWidget:      accessoryCircular
///
/// XCUITest can't place real Lock Screen widgets, so we render the identical
/// SwiftUI views inside the host app and assert on their accessibility
/// identifiers — the closest deterministic, CI-runnable stand-in for a widget
/// snapshot.
///
/// `XCUIApplication` and its query/element APIs are `@MainActor`-isolated.
/// `XCTestCase.setUpWithError()` is `nonisolated`, so an override of it cannot
/// touch the app; instead each `@MainActor` test launches the harness via the
/// shared `launchHarness()` helper.
@MainActor
final class WidgetSnapshotUITests: XCTestCase {
    private func launchHarness() -> XCUIApplication {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launchArguments = ["-WidgetSnapshotHarness"]
        app.launch()
        // The harness marker is a plain Text, so it surfaces as a staticText.
        XCTAssertTrue(
            app.staticTexts["widgetSnapshotHarness"].waitForExistence(timeout: 15)
                || app.descendants(matching: .any)["widgetSnapshotHarness"].waitForExistence(timeout: 5),
            "Widget snapshot harness should appear under the launch argument"
        )
        return app
    }

    func testHarnessIsPresented() {
        _ = launchHarness()
    }

    func testTodayCheckinWidgetRendersBothFamilies() {
        let app = launchHarness()
        XCTAssertTrue(
            elementExists("todayCheckinCircular", in: app),
            "TodayCheckinWidget accessoryCircular should render"
        )
        XCTAssertTrue(
            elementExists("todayCheckinRectangular", in: app),
            "TodayCheckinWidget accessoryRectangular should render"
        )
    }

    func testHeroMetricWidgetRendersBothFamilies() {
        let app = launchHarness()
        XCTAssertTrue(
            elementExists("heroMetricRectangular", in: app),
            "HeroMetricWidget accessoryRectangular should render"
        )
        XCTAssertTrue(
            elementExists("heroMetricSmall", in: app),
            "HeroMetricWidget systemSmall should render"
        )
    }

    func testQuickLogWidgetRendersCircular() {
        let app = launchHarness()
        XCTAssertTrue(
            elementExists("quickLogCircular", in: app),
            "QuickLogWidget accessoryCircular should render"
        )
    }

    /// An identifier may resolve to any element type depending on how SwiftUI
    /// lays it out, so check across the common query roots.
    private func elementExists(_ identifier: String, in app: XCUIApplication) -> Bool {
        let candidates: [XCUIElementQuery] = [
            app.otherElements,
            app.images,
            app.staticTexts,
            app.groups,
        ]
        for query in candidates where query[identifier].waitForExistence(timeout: 5) {
            return true
        }
        // Fall back to a descendant search across all element types.
        return app.descendants(matching: .any)[identifier].exists
    }
}
