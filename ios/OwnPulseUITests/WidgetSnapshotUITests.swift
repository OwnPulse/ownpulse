// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import XCTest

/// Drives the DEBUG-only `WidgetSnapshotHarness` (launched via the
/// `-WidgetSnapshotHarness` argument) and asserts that each widget view
/// renders for every supported family:
///   - TodayCheckinWidget: accessoryCircular + accessoryRectangular
///   - HeroMetricWidget:    accessoryRectangular + systemSmall
///   - QuickLogWidget:      accessoryCircular
///
/// XCUITest can't place real Lock Screen widgets, so we render the identical
/// SwiftUI views inside the host app via `WidgetPreviewContext` and assert on
/// their accessibility identifiers — the closest deterministic, CI-runnable
/// stand-in for a widget snapshot.
final class WidgetSnapshotUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["-WidgetSnapshotHarness"]
        app.launch()
    }

    func testHarnessIsPresented() throws {
        XCTAssertTrue(
            app.otherElements["widgetSnapshotHarness"].waitForExistence(timeout: 10),
            "Widget snapshot harness should appear under the launch argument"
        )
    }

    func testTodayCheckinWidgetRendersBothFamilies() throws {
        XCTAssertTrue(
            app.otherElements["widgetSnapshotHarness"].waitForExistence(timeout: 10)
        )
        XCTAssertTrue(
            elementExists("todayCheckinCircular"),
            "TodayCheckinWidget accessoryCircular should render"
        )
        XCTAssertTrue(
            elementExists("todayCheckinRectangular"),
            "TodayCheckinWidget accessoryRectangular should render"
        )
    }

    func testHeroMetricWidgetRendersBothFamilies() throws {
        XCTAssertTrue(
            app.otherElements["widgetSnapshotHarness"].waitForExistence(timeout: 10)
        )
        XCTAssertTrue(
            elementExists("heroMetricRectangular"),
            "HeroMetricWidget accessoryRectangular should render"
        )
        XCTAssertTrue(
            elementExists("heroMetricSmall"),
            "HeroMetricWidget systemSmall should render"
        )
    }

    func testQuickLogWidgetRendersCircular() throws {
        XCTAssertTrue(
            app.otherElements["widgetSnapshotHarness"].waitForExistence(timeout: 10)
        )
        XCTAssertTrue(
            elementExists("quickLogCircular"),
            "QuickLogWidget accessoryCircular should render"
        )
    }

    /// An identifier may resolve to any element type depending on how SwiftUI
    /// lays it out, so check across the common query roots.
    private func elementExists(_ identifier: String) -> Bool {
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
