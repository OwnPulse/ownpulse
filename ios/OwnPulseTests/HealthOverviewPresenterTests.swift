// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Foundation
import Testing
@testable import OwnPulse

@Suite("HealthOverviewPresenter")
struct HealthOverviewPresenterTests {
    // MARK: - humanLabel

    @Test("snake_case field -> Capitalized Words")
    func humanLabelPassthrough() {
        #expect(HealthOverviewPresenter.humanLabel(for: "heart_rate", prefs: .pounds) == "Heart Rate")
        #expect(
            HealthOverviewPresenter.humanLabel(for: "sleep_analysis", prefs: .pounds)
                == "Sleep Analysis"
        )
    }

    @Test("body_mass gets (lb) suffix when pref is pounds")
    func humanLabelBodyMassPounds() {
        let label = HealthOverviewPresenter.humanLabel(for: "body_mass", prefs: .pounds)
        #expect(label == "Body Mass (lb)")
    }

    @Test("body_mass gets (kg) suffix when pref is kilograms")
    func humanLabelBodyMassKilograms() {
        let label = HealthOverviewPresenter.humanLabel(for: "body_mass", prefs: .kilograms)
        #expect(label == "Body Mass (kg)")
    }

    // MARK: - uniqueSubstances

    private func marker(_ substance: String) -> InterventionMarker {
        InterventionMarker(
            t: "2026-04-10T00:00:00Z",
            substance: substance,
            dose: nil,
            unit: nil,
            route: nil
        )
    }

    @Test("returns sorted, de-duplicated substance names")
    func uniqueSubstancesDeduplicates() {
        let markers = [
            marker("Caffeine"),
            marker("Caffeine"),
            marker("Modafinil"),
            marker("Aspirin"),
            marker("Aspirin"),
        ]
        let substances = HealthOverviewPresenter.uniqueSubstances(from: markers)
        #expect(substances == ["Aspirin", "Caffeine", "Modafinil"])
    }

    @Test("empty interventions -> empty list")
    func uniqueSubstancesEmpty() {
        #expect(HealthOverviewPresenter.uniqueSubstances(from: []).isEmpty)
    }

    // MARK: - toggleHidden

    @Test("toggling a substance into an empty set adds it")
    func toggleHiddenAdd() {
        let next = HealthOverviewPresenter.toggleHidden("Caffeine", in: [])
        #expect(next == ["Caffeine"])
    }

    @Test("toggling a substance that is hidden removes it")
    func toggleHiddenRemove() {
        let start: Set<String> = ["Caffeine", "Modafinil"]
        let next = HealthOverviewPresenter.toggleHidden("Caffeine", in: start)
        #expect(next == ["Modafinil"])
    }

    @Test("toggling does not mutate other substances in the set")
    func toggleHiddenLeavesOthersAlone() {
        let start: Set<String> = ["Caffeine"]
        let next = HealthOverviewPresenter.toggleHidden("Modafinil", in: start)
        #expect(next == ["Caffeine", "Modafinil"])
    }
}
