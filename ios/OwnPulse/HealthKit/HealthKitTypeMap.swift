// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import HealthKit

enum HealthKitTypeMap {
    struct Mapping {
        let hkType: HKSampleType
        let recordType: String
        let unit: HKUnit
        let unitString: String
    }

    static let mappings: [Mapping] = [
        Mapping(
            hkType: HKQuantityType(.heartRate),
            recordType: "heart_rate",
            unit: HKUnit.count().unitDivided(by: .minute()),
            unitString: "bpm"
        ),
        Mapping(
            hkType: HKQuantityType(.restingHeartRate),
            recordType: "resting_heart_rate",
            unit: HKUnit.count().unitDivided(by: .minute()),
            unitString: "bpm"
        ),
        Mapping(
            hkType: HKQuantityType(.heartRateVariabilitySDNN),
            recordType: "heart_rate_variability",
            unit: HKUnit.secondUnit(with: .milli),
            unitString: "ms"
        ),
        Mapping(
            hkType: HKQuantityType(.stepCount),
            recordType: "steps",
            unit: .count(),
            unitString: "count"
        ),
        Mapping(
            hkType: HKQuantityType(.bodyMass),
            recordType: "body_mass",
            unit: .gramUnit(with: .kilo),
            unitString: "kg"
        ),
        Mapping(
            hkType: HKQuantityType(.bodyTemperature),
            recordType: "body_temperature",
            unit: .degreeCelsius(),
            unitString: "degC"
        ),
        Mapping(
            hkType: HKQuantityType(.oxygenSaturation),
            recordType: "blood_oxygen",
            unit: .percent(),
            unitString: "%"
        ),
        Mapping(
            hkType: HKQuantityType(.respiratoryRate),
            recordType: "respiratory_rate",
            unit: HKUnit.count().unitDivided(by: .minute()),
            unitString: "breaths/min"
        ),
        Mapping(
            hkType: HKQuantityType(.bloodPressureSystolic),
            recordType: "blood_pressure_systolic",
            unit: .millimeterOfMercury(),
            unitString: "mmHg"
        ),
        Mapping(
            hkType: HKQuantityType(.bloodPressureDiastolic),
            recordType: "blood_pressure_diastolic",
            unit: .millimeterOfMercury(),
            unitString: "mmHg"
        ),
        Mapping(
            hkType: HKCategoryType(.sleepAnalysis),
            recordType: "sleep_analysis",
            unit: .minute(),
            unitString: "min"
        ),
    ]

    static func mapping(forRecordType recordType: String) -> Mapping? {
        mappings.first { $0.recordType == recordType }
    }

    static func mapping(forHKType hkType: HKSampleType) -> Mapping? {
        mappings.first { $0.hkType == hkType }
    }

    static var allHKTypes: Set<HKSampleType> {
        Set(mappings.map(\.hkType))
    }

    static var allReadTypes: Set<HKObjectType> {
        Set(mappings.map { $0.hkType as HKObjectType })
    }

    static var allWriteTypes: Set<HKSampleType> {
        allHKTypes
    }
}
