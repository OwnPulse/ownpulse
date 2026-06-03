// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import Testing
@testable import OwnPulse

/// Covers the keyed per-metric chart-color lookup (`ChartColors`, generated
/// from the canonical token source). A known metric must resolve to its
/// dedicated token color regardless of index; an unknown metric must fall back
/// to the deterministic cycle. The same metric resolves to the same color on
/// web and iOS — that parity is enforced by the token generator and its web
/// vitest test; here we lock the iOS side of the lookup.
@Suite("ChartColors")
struct ChartColorsTests {
    /// The ACTUAL backend `record_type` field strings the explore API emits for
    /// each token-keyed metric (backend/api/src/models/explore.rs). These — not
    /// the token keys — are what `color(for:index:)` receives in production.
    static let fieldToTokenKey: [(field: String, tokenKey: String)] = [
        ("heart_rate", "heart_rate"),
        ("resting_heart_rate", "heart_rate"),
        ("heart_rate_variability", "hrv"),
        ("blood_pressure_systolic", "bp_systolic"),
        ("blood_pressure_diastolic", "bp_diastolic"),
        ("blood_glucose", "glucose"),
        ("body_mass", "weight"),
        ("sleep_analysis", "sleep_duration"),
    ]

    @Test("real backend field names resolve to their token color regardless of index")
    func realFieldsResolve() {
        for (field, tokenKey) in Self.fieldToTokenKey {
            let expected = ChartColors.metric[tokenKey]
            #expect(expected != nil)
            #expect(ChartColors.color(for: field, index: 0) == expected)
            #expect(ChartColors.color(for: field, index: 5) == expected)
            #expect(ChartColors.color(for: field, index: 99) == expected)
        }
    }

    @Test("glucose and the blood-pressure fields get distinct, dedicated colors")
    func bloodFieldsAreNotFallback() {
        // Regression guard: iOS previously had no alias layer, so these three
        // (plus hrv/body_mass/sleep_analysis) fell through to the fallback cycle.
        #expect(ChartColors.color(for: "blood_glucose", index: 0) == ChartColors.metric["glucose"])
        #expect(
            ChartColors.color(for: "blood_pressure_systolic", index: 0)
                == ChartColors.metric["bp_systolic"]
        )
        #expect(
            ChartColors.color(for: "blood_pressure_diastolic", index: 0)
                == ChartColors.metric["bp_diastolic"]
        )
        #expect(ChartColors.color(for: "blood_glucose", index: 0) != ChartColors.fallback[0])
    }

    @Test("canonical token keys also resolve (no alias needed)")
    func canonicalKeysResolve() {
        for metric in ChartColors.metric.keys {
            let expected = ChartColors.metric[metric]
            #expect(ChartColors.color(for: metric, index: 0) == expected)
            #expect(ChartColors.color(for: metric, index: 99) == expected)
        }
    }

    @Test("every metric color is reachable from a real backend field")
    func everyColorReachable() {
        var reachable = Set<String>()
        for pair in Self.fieldToTokenKey { reachable.insert(pair.tokenKey) }
        for tokenKey in ChartColors.aliases.values { reachable.insert(tokenKey) }
        for key in ChartColors.metric.keys {
            #expect(reachable.contains(key), "metric color \(key) is unreachable from any field")
        }
    }

    @Test("every generated alias is exercised by a known backend field")
    func aliasesAreExercised() {
        let knownFields = Set(Self.fieldToTokenKey.map(\.field))
        for field in ChartColors.aliases.keys {
            #expect(knownFields.contains(field), "alias \(field) maps a field the backend never emits")
        }
    }

    @Test("heart_rate matches the token color #d55e00")
    func heartRateMatchesToken() {
        let expected = Color(red: 213 / 255, green: 94 / 255, blue: 0 / 255)
        #expect(ChartColors.color(for: "heart_rate", index: 3) == expected)
    }

    @Test("unknown metrics fall back to the deterministic cycle by index")
    func unknownMetricFallsBack() {
        #expect(ChartColors.color(for: "unknown", index: 0) == ChartColors.fallback[0])
        #expect(ChartColors.color(for: "unknown", index: 1) == ChartColors.fallback[1])
    }

    @Test("the fallback cycle wraps around")
    func fallbackWraps() {
        let n = ChartColors.fallback.count
        #expect(ChartColors.color(for: "unknown", index: n) == ChartColors.fallback[0])
        #expect(ChartColors.color(for: "unknown", index: n + 2) == ChartColors.fallback[2])
    }
}
