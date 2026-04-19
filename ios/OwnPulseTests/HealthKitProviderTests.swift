// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import HealthKit
import Testing
@testable import OwnPulse

@Suite("HealthKitProvider — background-delivery frequency policy")
struct HealthKitProviderFrequencyTests {
    // This suite pins the record-type → frequency mapping so that adding a
    // new HealthKit mapping can't silently inherit the wrong policy. When
    // you add a new record type to HealthKitTypeMap, decide explicitly
    // whether it should be `.immediate` (low-latency events like heart
    // rate) or `.hourly` (bulk/aggregate metrics), and update these tests.

    @Test("heart_rate uses .immediate for low-latency workout updates")
    func heartRateIsImmediate() {
        let frequency = HealthKitProvider.backgroundDeliveryFrequency(for: "heart_rate")
        #expect(frequency == .immediate)
    }

    @Test("blood_oxygen uses .immediate for SpO2 spike detection")
    func bloodOxygenIsImmediate() {
        let frequency = HealthKitProvider.backgroundDeliveryFrequency(for: "blood_oxygen")
        #expect(frequency == .immediate)
    }

    @Test("steps uses .hourly to stay gentle on the battery")
    func stepsIsHourly() {
        let frequency = HealthKitProvider.backgroundDeliveryFrequency(for: "steps")
        #expect(frequency == .hourly)
    }

    @Test("sleep_analysis uses .hourly — sleep sessions are not latency-critical")
    func sleepIsHourly() {
        let frequency = HealthKitProvider.backgroundDeliveryFrequency(for: "sleep_analysis")
        #expect(frequency == .hourly)
    }

    @Test("unknown record types default to .hourly")
    func unknownDefaultsToHourly() {
        let frequency = HealthKitProvider.backgroundDeliveryFrequency(for: "some_hypothetical_future_type")
        #expect(frequency == .hourly)
    }

    @Test("all existing mappings resolve to one of the two allowed frequencies")
    func allMappingsResolve() {
        // Guard rail: if someone accidentally adds a third frequency bucket
        // the policy grows silently. Pin the allowed set here.
        for mapping in HealthKitTypeMap.mappings {
            let frequency = HealthKitProvider.backgroundDeliveryFrequency(for: mapping.recordType)
            #expect(
                frequency == .immediate || frequency == .hourly,
                "Unexpected frequency for \(mapping.recordType)"
            )
        }
    }

    @Test("immediate set contains exactly the documented record types")
    func immediateSetIsPinned() {
        // If this test fails, someone added a new `.immediate` type without
        // updating the documented rationale. Update either the set or the
        // tests — don't silently expand `.immediate` and drain the battery.
        #expect(HealthKitProvider.immediateDeliveryRecordTypes == ["heart_rate", "blood_oxygen"])
    }
}
