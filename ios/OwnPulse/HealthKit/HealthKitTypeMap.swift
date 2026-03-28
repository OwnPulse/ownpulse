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
            hkType: HKQuantityType(.activeEnergyBurned),
            recordType: "active_energy",
            unit: .kilocalorie(),
            unitString: "kcal"
        ),
        Mapping(
            hkType: HKQuantityType(.basalEnergyBurned),
            recordType: "basal_energy",
            unit: .kilocalorie(),
            unitString: "kcal"
        ),
        Mapping(
            hkType: HKQuantityType(.distanceWalkingRunning),
            recordType: "distance_walking_running",
            unit: .meter(),
            unitString: "m"
        ),
        Mapping(
            hkType: HKQuantityType(.flightsClimbed),
            recordType: "flights_climbed",
            unit: .count(),
            unitString: "count"
        ),
        Mapping(
            hkType: HKQuantityType(.appleExerciseTime),
            recordType: "exercise_time",
            unit: .minute(),
            unitString: "min"
        ),
        Mapping(
            hkType: HKQuantityType(.appleStandTime),
            recordType: "stand_time",
            unit: .minute(),
            unitString: "min"
        ),
        Mapping(
            hkType: HKQuantityType(.vo2Max),
            recordType: "vo2_max",
            unit: HKUnit(from: "mL/kg*min"),
            unitString: "mL/kg/min"
        ),
        Mapping(
            hkType: HKQuantityType(.bodyFatPercentage),
            recordType: "body_fat_percentage",
            unit: .percent(),
            unitString: "%"
        ),
        Mapping(
            hkType: HKQuantityType(.leanBodyMass),
            recordType: "lean_body_mass",
            unit: .gramUnit(with: .kilo),
            unitString: "kg"
        ),
        Mapping(
            hkType: HKQuantityType(.height),
            recordType: "height",
            unit: .meterUnit(with: .centi),
            unitString: "cm"
        ),
        Mapping(
            hkType: HKQuantityType(.waistCircumference),
            recordType: "waist_circumference",
            unit: .meterUnit(with: .centi),
            unitString: "cm"
        ),
        Mapping(
            hkType: HKQuantityType(.walkingSpeed),
            recordType: "walking_speed",
            unit: HKUnit.meter().unitDivided(by: .second()),
            unitString: "m/s"
        ),
        Mapping(
            hkType: HKQuantityType(.distanceCycling),
            recordType: "distance_cycling",
            unit: .meter(),
            unitString: "m"
        ),
        Mapping(
            hkType: HKQuantityType(.distanceSwimming),
            recordType: "distance_swimming",
            unit: .meter(),
            unitString: "m"
        ),
        Mapping(
            hkType: HKQuantityType(.swimmingStrokeCount),
            recordType: "swimming_strokes",
            unit: .count(),
            unitString: "count"
        ),
        Mapping(
            hkType: HKQuantityType(.dietaryWater),
            recordType: "water_intake",
            unit: .liter(),
            unitString: "L"
        ),
        Mapping(
            hkType: HKQuantityType(.dietaryEnergyConsumed),
            recordType: "dietary_energy",
            unit: .kilocalorie(),
            unitString: "kcal"
        ),
        Mapping(
            hkType: HKQuantityType(.bodyMassIndex),
            recordType: "bmi",
            unit: .count(),
            unitString: "count"
        ),
        Mapping(
            hkType: HKQuantityType(.walkingHeartRateAverage),
            recordType: "walking_heart_rate",
            unit: HKUnit.count().unitDivided(by: .minute()),
            unitString: "bpm"
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
