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

        // MARK: - Vitals / metabolic

        Mapping(
            hkType: HKQuantityType(.bloodGlucose),
            recordType: "blood_glucose",
            unit: HKUnit(from: "mg/dL"),
            unitString: "mg/dL"
        ),
        Mapping(
            hkType: HKQuantityType(.insulinDelivery),
            recordType: "insulin_delivery",
            unit: .internationalUnit(),
            unitString: "IU"
        ),
        Mapping(
            hkType: HKQuantityType(.appleSleepingWristTemperature),
            recordType: "sleeping_wrist_temperature",
            unit: .degreeCelsius(),
            unitString: "degC"
        ),

        // MARK: - Running

        Mapping(
            hkType: HKQuantityType(.runningSpeed),
            recordType: "running_speed",
            unit: HKUnit.meter().unitDivided(by: .second()),
            unitString: "m/s"
        ),
        Mapping(
            hkType: HKQuantityType(.runningPower),
            recordType: "running_power",
            unit: .watt(),
            unitString: "W"
        ),
        Mapping(
            hkType: HKQuantityType(.runningStrideLength),
            recordType: "running_stride_length",
            unit: .meter(),
            unitString: "m"
        ),
        Mapping(
            hkType: HKQuantityType(.runningVerticalOscillation),
            recordType: "running_vertical_oscillation",
            unit: .meterUnit(with: .centi),
            unitString: "cm"
        ),
        Mapping(
            hkType: HKQuantityType(.runningGroundContactTime),
            recordType: "running_ground_contact_time",
            unit: .secondUnit(with: .milli),
            unitString: "ms"
        ),

        // MARK: - Cycling

        Mapping(
            hkType: HKQuantityType(.cyclingSpeed),
            recordType: "cycling_speed",
            unit: HKUnit.meter().unitDivided(by: .second()),
            unitString: "m/s"
        ),
        Mapping(
            hkType: HKQuantityType(.cyclingPower),
            recordType: "cycling_power",
            unit: .watt(),
            unitString: "W"
        ),
        Mapping(
            hkType: HKQuantityType(.cyclingCadence),
            recordType: "cycling_cadence",
            unit: HKUnit.count().unitDivided(by: .minute()),
            unitString: "rpm"
        ),
        Mapping(
            hkType: HKQuantityType(.cyclingFunctionalThresholdPower),
            recordType: "cycling_ftp",
            unit: .watt(),
            unitString: "W"
        ),

        // MARK: - Mobility

        Mapping(
            hkType: HKQuantityType(.walkingDoubleSupportPercentage),
            recordType: "walking_double_support",
            unit: .percent(),
            unitString: "%"
        ),
        Mapping(
            hkType: HKQuantityType(.walkingStepLength),
            recordType: "walking_step_length",
            unit: .meterUnit(with: .centi),
            unitString: "cm"
        ),
        Mapping(
            hkType: HKQuantityType(.walkingAsymmetryPercentage),
            recordType: "walking_asymmetry",
            unit: .percent(),
            unitString: "%"
        ),
        Mapping(
            hkType: HKQuantityType(.stairAscentSpeed),
            recordType: "stair_ascent_speed",
            unit: HKUnit.meter().unitDivided(by: .second()),
            unitString: "m/s"
        ),
        Mapping(
            hkType: HKQuantityType(.stairDescentSpeed),
            recordType: "stair_descent_speed",
            unit: HKUnit.meter().unitDivided(by: .second()),
            unitString: "m/s"
        ),
        Mapping(
            hkType: HKQuantityType(.sixMinuteWalkTestDistance),
            recordType: "six_min_walk_distance",
            unit: .meter(),
            unitString: "m"
        ),

        // MARK: - Activity

        Mapping(
            hkType: HKQuantityType(.appleMoveTime),
            recordType: "move_time",
            unit: .minute(),
            unitString: "min"
        ),
        Mapping(
            hkType: HKQuantityType(.physicalEffort),
            recordType: "physical_effort",
            unit: HKUnit(from: "kcal/hr*kg"),
            unitString: "kcal/hr/kg"
        ),

        // MARK: - Environment

        Mapping(
            hkType: HKQuantityType(.timeInDaylight),
            recordType: "time_in_daylight",
            unit: .minute(),
            unitString: "min"
        ),
        Mapping(
            hkType: HKQuantityType(.environmentalAudioExposure),
            recordType: "environmental_audio",
            unit: .decibelAWeightedSoundPressureLevel(),
            unitString: "dBASPL"
        ),
        Mapping(
            hkType: HKQuantityType(.headphoneAudioExposure),
            recordType: "headphone_audio",
            unit: .decibelAWeightedSoundPressureLevel(),
            unitString: "dBASPL"
        ),
        Mapping(
            hkType: HKQuantityType(.numberOfTimesFallen),
            recordType: "falls",
            unit: .count(),
            unitString: "count"
        ),

        // MARK: - Dietary

        Mapping(
            hkType: HKQuantityType(.dietaryProtein),
            recordType: "dietary_protein",
            unit: .gram(),
            unitString: "g"
        ),
        Mapping(
            hkType: HKQuantityType(.dietaryFatTotal),
            recordType: "dietary_fat",
            unit: .gram(),
            unitString: "g"
        ),
        Mapping(
            hkType: HKQuantityType(.dietaryCarbohydrates),
            recordType: "dietary_carbs",
            unit: .gram(),
            unitString: "g"
        ),
        Mapping(
            hkType: HKQuantityType(.dietaryFiber),
            recordType: "dietary_fiber",
            unit: .gram(),
            unitString: "g"
        ),
        Mapping(
            hkType: HKQuantityType(.dietarySugar),
            recordType: "dietary_sugar",
            unit: .gram(),
            unitString: "g"
        ),
        Mapping(
            hkType: HKQuantityType(.dietaryCaffeine),
            recordType: "dietary_caffeine",
            unit: .gramUnit(with: .milli),
            unitString: "mg"
        ),
        Mapping(
            hkType: HKQuantityType(.dietarySodium),
            recordType: "dietary_sodium",
            unit: .gramUnit(with: .milli),
            unitString: "mg"
        ),
        Mapping(
            hkType: HKQuantityType(.dietaryCholesterol),
            recordType: "dietary_cholesterol",
            unit: .gramUnit(with: .milli),
            unitString: "mg"
        ),
        Mapping(
            hkType: HKQuantityType(.dietaryIron),
            recordType: "dietary_iron",
            unit: .gramUnit(with: .milli),
            unitString: "mg"
        ),
        Mapping(
            hkType: HKQuantityType(.dietaryVitaminC),
            recordType: "dietary_vitamin_c",
            unit: .gramUnit(with: .milli),
            unitString: "mg"
        ),
        Mapping(
            hkType: HKQuantityType(.dietaryVitaminD),
            recordType: "dietary_vitamin_d",
            unit: .internationalUnit(),
            unitString: "IU"
        ),
        Mapping(
            hkType: HKQuantityType(.dietaryCalcium),
            recordType: "dietary_calcium",
            unit: .gramUnit(with: .milli),
            unitString: "mg"
        ),
        Mapping(
            hkType: HKQuantityType(.dietaryPotassium),
            recordType: "dietary_potassium",
            unit: .gramUnit(with: .milli),
            unitString: "mg"
        ),
        Mapping(
            hkType: HKQuantityType(.dietaryZinc),
            recordType: "dietary_zinc",
            unit: .gramUnit(with: .milli),
            unitString: "mg"
        ),
        Mapping(
            hkType: HKQuantityType(.dietaryMagnesium),
            recordType: "dietary_magnesium",
            unit: .gramUnit(with: .milli),
            unitString: "mg"
        ),

        // MARK: - Category types

        Mapping(
            hkType: HKCategoryType(.mindfulSession),
            recordType: "mindful_session",
            unit: .minute(),
            unitString: "min"
        ),
        Mapping(
            hkType: HKCategoryType(.highHeartRateEvent),
            recordType: "high_heart_rate_event",
            unit: .count(),
            unitString: "count"
        ),
        Mapping(
            hkType: HKCategoryType(.lowHeartRateEvent),
            recordType: "low_heart_rate_event",
            unit: .count(),
            unitString: "count"
        ),
        Mapping(
            hkType: HKCategoryType(.irregularHeartRhythmEvent),
            recordType: "irregular_heart_rhythm_event",
            unit: .count(),
            unitString: "count"
        ),
        Mapping(
            hkType: HKCategoryType(.appleStandHour),
            recordType: "stand_hour",
            unit: .count(),
            unitString: "count"
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
