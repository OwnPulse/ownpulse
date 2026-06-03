// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import SwiftUI
import Testing
@testable import OwnPulse

@Suite("SmallMultiplesChartView helpers")
struct SmallMultiplesChartViewTests {
    private func metric(_ field: String, unit: String, points: Int = 3) -> ChartMetric {
        let chartPoints = (0..<points).map {
            ChartPoint(date: Date(timeIntervalSince1970: Double($0) * 86_400), value: Double($0))
        }
        return ChartMetric(
            field: field,
            label: field.replacingOccurrences(of: "_", with: " ").capitalized,
            unit: unit,
            color: .red,
            points: chartPoints,
            maPoints: nil
        )
    }

    private func marker(_ substance: String) -> InterventionMarker {
        InterventionMarker(
            t: "2026-04-10T00:00:00Z",
            substance: substance,
            dose: nil,
            unit: nil,
            route: nil
        )
    }

    // MARK: - visible interventions filter

    @Test("hidden substances are filtered out of the interventions list")
    func filterVisibleInterventionsHidesMatches() {
        let interventions = [marker("Caffeine"), marker("Modafinil"), marker("Aspirin")]
        let visible = SmallMultiplesChartView.filterVisibleInterventions(
            interventions,
            hiddenSubstances: ["Caffeine"]
        )
        #expect(visible.count == 2)
        #expect(visible.map(\.substance).sorted() == ["Aspirin", "Modafinil"])
    }

    @Test("empty hidden set returns all interventions")
    func filterVisibleInterventionsNoHidden() {
        let interventions = [marker("Caffeine"), marker("Modafinil")]
        let visible = SmallMultiplesChartView.filterVisibleInterventions(
            interventions,
            hiddenSubstances: []
        )
        #expect(visible.count == 2)
    }

    @Test("hiding every substance produces an empty list")
    func filterVisibleInterventionsAllHidden() {
        let interventions = [marker("Caffeine"), marker("Modafinil")]
        let visible = SmallMultiplesChartView.filterVisibleInterventions(
            interventions,
            hiddenSubstances: ["Caffeine", "Modafinil"]
        )
        #expect(visible.isEmpty)
    }

    // MARK: - body_mass unit label

    @Test("body_mass panel label uses WeightFormatter (pounds)")
    func unitLabelBodyMassPounds() {
        let m = metric("body_mass", unit: "kg")
        let label = SmallMultiplesChartView.unitLabel(for: m, prefs: .pounds)
        #expect(label == "lb")
    }

    @Test("body_mass panel label uses WeightFormatter (kilograms)")
    func unitLabelBodyMassKilograms() {
        let m = metric("body_mass", unit: "kg")
        let label = SmallMultiplesChartView.unitLabel(for: m, prefs: .kilograms)
        #expect(label == "kg")
    }

    @Test("non-body_mass metrics pass their backend unit through")
    func unitLabelPassthrough() {
        #expect(
            SmallMultiplesChartView.unitLabel(
                for: metric("heart_rate", unit: "bpm"),
                prefs: .pounds
            ) == "bpm"
        )
        #expect(
            SmallMultiplesChartView.unitLabel(
                for: metric("sleep_analysis", unit: "min"),
                prefs: .pounds
            ) == "min"
        )
    }

    // MARK: - panel accessibility value

    @Test("panel accessibility value spells out the value range and unit")
    func panelAccessibilityValueDescribesRange() {
        // points are value 0, 1, 2 for a 3-point metric
        let value = SmallMultiplesChartView.panelAccessibilityValue(
            for: metric("heart_rate", unit: "bpm"),
            prefs: .kilograms
        )
        #expect(value == "From 0.0 to 2.0 bpm")
    }

    @Test("panel accessibility value applies weight-unit preference for body_mass")
    func panelAccessibilityValueBodyMassUsesPrefUnit() {
        let value = SmallMultiplesChartView.panelAccessibilityValue(
            for: metric("body_mass", unit: "kg"),
            prefs: .pounds
        )
        #expect(value.hasSuffix("lb"))
    }

    @Test("panel accessibility value reports no data for an empty series")
    func panelAccessibilityValueEmpty() {
        let value = SmallMultiplesChartView.panelAccessibilityValue(
            for: metric("heart_rate", unit: "bpm", points: 0),
            prefs: .kilograms
        )
        #expect(value == "No data")
    }

    // MARK: - instantiation smoke test

    @Test("view instantiates with 3 metrics without crashing")
    @MainActor
    func viewInstantiates() {
        // Not a rendering test — but exercising the initializer guards
        // against a change that breaks the public constructor shape.
        let view = SmallMultiplesChartView(
            metrics: [
                metric("body_mass", unit: "kg"),
                metric("heart_rate", unit: "bpm"),
                metric("sleep_analysis", unit: "min"),
            ],
            interventions: [marker("Caffeine")],
            hiddenSubstances: [],
            panelHeight: 120,
            showMovingAverage: false
        )
        #expect(view.metrics.count == 3)
    }
}
