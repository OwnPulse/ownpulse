// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import HealthKit
import Testing
@testable import OwnPulse

@Suite("HealthKitTypeMap")
struct HealthKitTypeMapTests {
    @Test("all mappings have unique record types")
    func uniqueRecordTypes() {
        let types = HealthKitTypeMap.mappings.map(\.recordType)
        #expect(Set(types).count == types.count)
    }

    @Test("all mappings have unique HK types")
    func uniqueHKTypes() {
        let types = HealthKitTypeMap.mappings.map(\.hkType)
        #expect(Set(types).count == types.count)
    }

    @Test("bidirectional lookup works for heart_rate")
    func bidirectionalHeartRate() {
        let byRecord = HealthKitTypeMap.mapping(forRecordType: "heart_rate")
        #expect(byRecord != nil)
        #expect(byRecord?.hkType == HKQuantityType(.heartRate))

        let byHK = HealthKitTypeMap.mapping(forHKType: HKQuantityType(.heartRate))
        #expect(byHK != nil)
        #expect(byHK?.recordType == "heart_rate")
    }

    @Test("allHKTypes contains expected count")
    func allTypesCount() {
        #expect(HealthKitTypeMap.allHKTypes.count == HealthKitTypeMap.mappings.count)
    }

    @Test("unknown record type returns nil")
    func unknownType() {
        #expect(HealthKitTypeMap.mapping(forRecordType: "nonexistent") == nil)
    }
}
