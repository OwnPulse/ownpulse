// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;

// ---------------------------------------------------------------------------
// Source-field enum allowlist — CRITICAL for security.
// No user input is ever interpolated into SQL. All field names come from these
// enums, and lab markers are always bind-parameterized.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum MetricSource {
    HealthRecord(HealthRecordField),
    Checkin(CheckinField),
    Lab(String),
    Calendar(CalendarField),
    Sleep(SleepField),
    ObserverPoll(ObserverPollField),
}

/// An observer poll metric field: the poll UUID and the dimension name.
#[derive(Debug, Clone)]
pub struct ObserverPollField {
    pub poll_id: Uuid,
    pub dimension: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aggregation {
    Avg,
    Sum,
    SleepDuration, // Special: SUM of segment durations where value IN (1,3,4,5)
    CountEvents,   // Special: COUNT of events per bucket
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthRecordField {
    // Vitals
    HeartRate,
    HeartRateVariability,
    RestingHeartRate,
    BloodPressureSystolic,
    BloodPressureDiastolic,
    BloodGlucose,
    BloodOxygen,
    RespiratoryRate,
    Vo2Max,
    WalkingHeartRate,
    InsulinDelivery,
    SleepingWristTemperature,

    // Body
    BodyMass,
    BodyFatPercentage,
    BodyTemperature,
    LeanBodyMass,
    Height,
    WaistCircumference,
    Bmi,

    // Activity
    Steps,
    ActiveEnergy,
    BasalEnergy,
    DistanceWalkingRunning,
    FlightsClimbed,
    ExerciseTime,
    StandTime,
    DistanceCycling,
    DistanceSwimming,
    SwimmingStrokes,
    MoveTime,
    PhysicalEffort,

    // Running
    RunningSpeed,
    RunningPower,
    RunningStrideLength,
    RunningVerticalOscillation,
    RunningGroundContactTime,

    // Cycling
    CyclingSpeed,
    CyclingPower,
    CyclingCadence,
    CyclingFtp,

    // Mobility
    WalkingSpeed,
    WalkingDoubleSupport,
    WalkingStepLength,
    WalkingAsymmetry,
    StairAscentSpeed,
    StairDescentSpeed,
    SixMinWalkDistance,

    // Sleep
    SleepAnalysis,

    // Dietary
    WaterIntake,
    DietaryEnergy,
    DietaryProtein,
    DietaryFat,
    DietaryCarbs,
    DietaryFiber,
    DietarySugar,
    DietaryCaffeine,
    DietarySodium,
    DietaryCholesterol,
    DietaryIron,
    DietaryVitaminC,
    DietaryVitaminD,
    DietaryCalcium,
    DietaryPotassium,
    DietaryZinc,
    DietaryMagnesium,

    // Environment
    TimeInDaylight,
    EnvironmentalAudio,
    HeadphoneAudio,

    // Events
    MindfulSession,
    HighHeartRateEvent,
    LowHeartRateEvent,
    IrregularHeartRhythmEvent,
    StandHour,
    Falls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckinField {
    Energy,
    Mood,
    Focus,
    Recovery,
    Libido,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarField {
    MeetingMinutes,
    MeetingCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepField {
    DurationMinutes,
    DeepMinutes,
    RemMinutes,
    Score,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Resolution {
    Daily,
    Weekly,
    Monthly,
}

impl Resolution {
    /// Return the PostgreSQL `date_trunc` interval string.
    pub fn pg_interval(&self) -> &'static str {
        match self {
            Resolution::Daily => "day",
            Resolution::Weekly => "week",
            Resolution::Monthly => "month",
        }
    }
}

impl HealthRecordField {
    /// The `record_type` string stored in `health_records.record_type`.
    pub fn record_type(&self) -> &'static str {
        match self {
            Self::HeartRate => "heart_rate",
            Self::HeartRateVariability => "heart_rate_variability",
            Self::RestingHeartRate => "resting_heart_rate",
            Self::BloodPressureSystolic => "blood_pressure_systolic",
            Self::BloodPressureDiastolic => "blood_pressure_diastolic",
            Self::BloodGlucose => "blood_glucose",
            Self::BloodOxygen => "blood_oxygen",
            Self::RespiratoryRate => "respiratory_rate",
            Self::Vo2Max => "vo2_max",
            Self::WalkingHeartRate => "walking_heart_rate",
            Self::InsulinDelivery => "insulin_delivery",
            Self::SleepingWristTemperature => "sleeping_wrist_temperature",
            Self::BodyMass => "body_mass",
            Self::BodyFatPercentage => "body_fat_percentage",
            Self::BodyTemperature => "body_temperature",
            Self::LeanBodyMass => "lean_body_mass",
            Self::Height => "height",
            Self::WaistCircumference => "waist_circumference",
            Self::Bmi => "bmi",
            Self::Steps => "steps",
            Self::ActiveEnergy => "active_energy",
            Self::BasalEnergy => "basal_energy",
            Self::DistanceWalkingRunning => "distance_walking_running",
            Self::FlightsClimbed => "flights_climbed",
            Self::ExerciseTime => "exercise_time",
            Self::StandTime => "stand_time",
            Self::DistanceCycling => "distance_cycling",
            Self::DistanceSwimming => "distance_swimming",
            Self::SwimmingStrokes => "swimming_strokes",
            Self::MoveTime => "move_time",
            Self::PhysicalEffort => "physical_effort",
            Self::RunningSpeed => "running_speed",
            Self::RunningPower => "running_power",
            Self::RunningStrideLength => "running_stride_length",
            Self::RunningVerticalOscillation => "running_vertical_oscillation",
            Self::RunningGroundContactTime => "running_ground_contact_time",
            Self::CyclingSpeed => "cycling_speed",
            Self::CyclingPower => "cycling_power",
            Self::CyclingCadence => "cycling_cadence",
            Self::CyclingFtp => "cycling_ftp",
            Self::WalkingSpeed => "walking_speed",
            Self::WalkingDoubleSupport => "walking_double_support",
            Self::WalkingStepLength => "walking_step_length",
            Self::WalkingAsymmetry => "walking_asymmetry",
            Self::StairAscentSpeed => "stair_ascent_speed",
            Self::StairDescentSpeed => "stair_descent_speed",
            Self::SixMinWalkDistance => "six_min_walk_distance",
            Self::SleepAnalysis => "sleep_analysis",
            Self::WaterIntake => "water_intake",
            Self::DietaryEnergy => "dietary_energy",
            Self::DietaryProtein => "dietary_protein",
            Self::DietaryFat => "dietary_fat",
            Self::DietaryCarbs => "dietary_carbs",
            Self::DietaryFiber => "dietary_fiber",
            Self::DietarySugar => "dietary_sugar",
            Self::DietaryCaffeine => "dietary_caffeine",
            Self::DietarySodium => "dietary_sodium",
            Self::DietaryCholesterol => "dietary_cholesterol",
            Self::DietaryIron => "dietary_iron",
            Self::DietaryVitaminC => "dietary_vitamin_c",
            Self::DietaryVitaminD => "dietary_vitamin_d",
            Self::DietaryCalcium => "dietary_calcium",
            Self::DietaryPotassium => "dietary_potassium",
            Self::DietaryZinc => "dietary_zinc",
            Self::DietaryMagnesium => "dietary_magnesium",
            Self::TimeInDaylight => "time_in_daylight",
            Self::EnvironmentalAudio => "environmental_audio",
            Self::HeadphoneAudio => "headphone_audio",
            Self::MindfulSession => "mindful_session",
            Self::HighHeartRateEvent => "high_heart_rate_event",
            Self::LowHeartRateEvent => "low_heart_rate_event",
            Self::IrregularHeartRhythmEvent => "irregular_heart_rhythm_event",
            Self::StandHour => "stand_hour",
            Self::Falls => "falls",
        }
    }

    /// Human-readable unit for this metric.
    pub fn unit(&self) -> &'static str {
        match self {
            Self::HeartRate | Self::RestingHeartRate | Self::WalkingHeartRate => "bpm",
            Self::HeartRateVariability => "ms",
            Self::BloodPressureSystolic | Self::BloodPressureDiastolic => "mmHg",
            Self::BloodGlucose => "mg/dL",
            Self::BloodOxygen | Self::BodyFatPercentage => "%",
            Self::RespiratoryRate => "breaths/min",
            Self::Vo2Max => "mL/kg/min",
            Self::InsulinDelivery | Self::DietaryVitaminD => "IU",
            Self::SleepingWristTemperature | Self::BodyTemperature => "\u{00b0}C",
            Self::BodyMass | Self::LeanBodyMass => "kg",
            Self::Height
            | Self::WaistCircumference
            | Self::WalkingStepLength
            | Self::RunningVerticalOscillation => "cm",
            Self::Bmi
            | Self::SwimmingStrokes
            | Self::Falls
            | Self::HighHeartRateEvent
            | Self::LowHeartRateEvent
            | Self::IrregularHeartRhythmEvent
            | Self::StandHour => "count",
            Self::Steps => "steps",
            Self::ActiveEnergy | Self::BasalEnergy | Self::DietaryEnergy => "kcal",
            Self::DistanceWalkingRunning
            | Self::DistanceCycling
            | Self::DistanceSwimming
            | Self::SixMinWalkDistance
            | Self::RunningStrideLength => "m",
            Self::FlightsClimbed => "floors",
            Self::ExerciseTime
            | Self::StandTime
            | Self::MoveTime
            | Self::SleepAnalysis
            | Self::MindfulSession
            | Self::TimeInDaylight => "min",
            Self::WalkingSpeed
            | Self::RunningSpeed
            | Self::CyclingSpeed
            | Self::StairAscentSpeed
            | Self::StairDescentSpeed => "m/s",
            Self::RunningPower | Self::CyclingPower | Self::CyclingFtp => "W",
            Self::RunningGroundContactTime => "ms",
            Self::CyclingCadence => "rpm",
            Self::WalkingDoubleSupport | Self::WalkingAsymmetry => "%",
            Self::PhysicalEffort => "kcal/hr/kg",
            Self::EnvironmentalAudio | Self::HeadphoneAudio => "dBASPL",
            Self::WaterIntake => "L",
            Self::DietaryProtein
            | Self::DietaryFat
            | Self::DietaryCarbs
            | Self::DietaryFiber
            | Self::DietarySugar => "g",
            Self::DietaryCaffeine
            | Self::DietarySodium
            | Self::DietaryCholesterol
            | Self::DietaryIron
            | Self::DietaryVitaminC
            | Self::DietaryCalcium
            | Self::DietaryPotassium
            | Self::DietaryZinc
            | Self::DietaryMagnesium => "mg",
        }
    }

    /// Parse a field name string into a `HealthRecordField`.
    pub fn parse(field: &str) -> Option<Self> {
        match field {
            "heart_rate" => Some(Self::HeartRate),
            "heart_rate_variability" => Some(Self::HeartRateVariability),
            "resting_heart_rate" => Some(Self::RestingHeartRate),
            "blood_pressure_systolic" => Some(Self::BloodPressureSystolic),
            "blood_pressure_diastolic" => Some(Self::BloodPressureDiastolic),
            "blood_glucose" => Some(Self::BloodGlucose),
            "blood_oxygen" => Some(Self::BloodOxygen),
            "respiratory_rate" => Some(Self::RespiratoryRate),
            "vo2_max" => Some(Self::Vo2Max),
            "walking_heart_rate" => Some(Self::WalkingHeartRate),
            "insulin_delivery" => Some(Self::InsulinDelivery),
            "sleeping_wrist_temperature" => Some(Self::SleepingWristTemperature),
            "body_mass" => Some(Self::BodyMass),
            "body_fat_percentage" => Some(Self::BodyFatPercentage),
            "body_temperature" => Some(Self::BodyTemperature),
            "lean_body_mass" => Some(Self::LeanBodyMass),
            "height" => Some(Self::Height),
            "waist_circumference" => Some(Self::WaistCircumference),
            "bmi" => Some(Self::Bmi),
            "steps" => Some(Self::Steps),
            "active_energy" => Some(Self::ActiveEnergy),
            "basal_energy" => Some(Self::BasalEnergy),
            "distance_walking_running" => Some(Self::DistanceWalkingRunning),
            "flights_climbed" => Some(Self::FlightsClimbed),
            "exercise_time" => Some(Self::ExerciseTime),
            "stand_time" => Some(Self::StandTime),
            "distance_cycling" => Some(Self::DistanceCycling),
            "distance_swimming" => Some(Self::DistanceSwimming),
            "swimming_strokes" => Some(Self::SwimmingStrokes),
            "move_time" => Some(Self::MoveTime),
            "physical_effort" => Some(Self::PhysicalEffort),
            "running_speed" => Some(Self::RunningSpeed),
            "running_power" => Some(Self::RunningPower),
            "running_stride_length" => Some(Self::RunningStrideLength),
            "running_vertical_oscillation" => Some(Self::RunningVerticalOscillation),
            "running_ground_contact_time" => Some(Self::RunningGroundContactTime),
            "cycling_speed" => Some(Self::CyclingSpeed),
            "cycling_power" => Some(Self::CyclingPower),
            "cycling_cadence" => Some(Self::CyclingCadence),
            "cycling_ftp" => Some(Self::CyclingFtp),
            "walking_speed" => Some(Self::WalkingSpeed),
            "walking_double_support" => Some(Self::WalkingDoubleSupport),
            "walking_step_length" => Some(Self::WalkingStepLength),
            "walking_asymmetry" => Some(Self::WalkingAsymmetry),
            "stair_ascent_speed" => Some(Self::StairAscentSpeed),
            "stair_descent_speed" => Some(Self::StairDescentSpeed),
            "six_min_walk_distance" => Some(Self::SixMinWalkDistance),
            "sleep_analysis" => Some(Self::SleepAnalysis),
            "water_intake" => Some(Self::WaterIntake),
            "dietary_energy" => Some(Self::DietaryEnergy),
            "dietary_protein" => Some(Self::DietaryProtein),
            "dietary_fat" => Some(Self::DietaryFat),
            "dietary_carbs" => Some(Self::DietaryCarbs),
            "dietary_fiber" => Some(Self::DietaryFiber),
            "dietary_sugar" => Some(Self::DietarySugar),
            "dietary_caffeine" => Some(Self::DietaryCaffeine),
            "dietary_sodium" => Some(Self::DietarySodium),
            "dietary_cholesterol" => Some(Self::DietaryCholesterol),
            "dietary_iron" => Some(Self::DietaryIron),
            "dietary_vitamin_c" => Some(Self::DietaryVitaminC),
            "dietary_vitamin_d" => Some(Self::DietaryVitaminD),
            "dietary_calcium" => Some(Self::DietaryCalcium),
            "dietary_potassium" => Some(Self::DietaryPotassium),
            "dietary_zinc" => Some(Self::DietaryZinc),
            "dietary_magnesium" => Some(Self::DietaryMagnesium),
            "time_in_daylight" => Some(Self::TimeInDaylight),
            "environmental_audio" => Some(Self::EnvironmentalAudio),
            "headphone_audio" => Some(Self::HeadphoneAudio),
            "mindful_session" => Some(Self::MindfulSession),
            "high_heart_rate_event" => Some(Self::HighHeartRateEvent),
            "low_heart_rate_event" => Some(Self::LowHeartRateEvent),
            "irregular_heart_rhythm_event" => Some(Self::IrregularHeartRhythmEvent),
            "stand_hour" => Some(Self::StandHour),
            "falls" => Some(Self::Falls),
            _ => None,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::HeartRate => "Heart Rate",
            Self::HeartRateVariability => "Heart Rate Variability",
            Self::RestingHeartRate => "Resting Heart Rate",
            Self::BloodPressureSystolic => "Blood Pressure (Systolic)",
            Self::BloodPressureDiastolic => "Blood Pressure (Diastolic)",
            Self::BloodGlucose => "Blood Glucose",
            Self::BloodOxygen => "Blood Oxygen",
            Self::RespiratoryRate => "Respiratory Rate",
            Self::Vo2Max => "VO2 Max",
            Self::WalkingHeartRate => "Walking Heart Rate",
            Self::InsulinDelivery => "Insulin Delivery",
            Self::SleepingWristTemperature => "Sleeping Wrist Temperature",
            Self::BodyMass => "Body Mass",
            Self::BodyFatPercentage => "Body Fat %",
            Self::BodyTemperature => "Body Temperature",
            Self::LeanBodyMass => "Lean Body Mass",
            Self::Height => "Height",
            Self::WaistCircumference => "Waist Circumference",
            Self::Bmi => "BMI",
            Self::Steps => "Steps",
            Self::ActiveEnergy => "Active Energy",
            Self::BasalEnergy => "Basal Energy",
            Self::DistanceWalkingRunning => "Distance (Walk/Run)",
            Self::FlightsClimbed => "Flights Climbed",
            Self::ExerciseTime => "Exercise Time",
            Self::StandTime => "Stand Time",
            Self::DistanceCycling => "Distance (Cycling)",
            Self::DistanceSwimming => "Distance (Swimming)",
            Self::SwimmingStrokes => "Swimming Strokes",
            Self::MoveTime => "Move Time",
            Self::PhysicalEffort => "Physical Effort",
            Self::RunningSpeed => "Running Speed",
            Self::RunningPower => "Running Power",
            Self::RunningStrideLength => "Running Stride Length",
            Self::RunningVerticalOscillation => "Running Vertical Oscillation",
            Self::RunningGroundContactTime => "Running Ground Contact Time",
            Self::CyclingSpeed => "Cycling Speed",
            Self::CyclingPower => "Cycling Power",
            Self::CyclingCadence => "Cycling Cadence",
            Self::CyclingFtp => "Cycling FTP",
            Self::WalkingSpeed => "Walking Speed",
            Self::WalkingDoubleSupport => "Walking Double Support",
            Self::WalkingStepLength => "Walking Step Length",
            Self::WalkingAsymmetry => "Walking Asymmetry",
            Self::StairAscentSpeed => "Stair Ascent Speed",
            Self::StairDescentSpeed => "Stair Descent Speed",
            Self::SixMinWalkDistance => "Six-Minute Walk Distance",
            Self::SleepAnalysis => "Sleep Duration (HealthKit)",
            Self::WaterIntake => "Water Intake",
            Self::DietaryEnergy => "Dietary Energy",
            Self::DietaryProtein => "Dietary Protein",
            Self::DietaryFat => "Dietary Fat",
            Self::DietaryCarbs => "Dietary Carbs",
            Self::DietaryFiber => "Dietary Fiber",
            Self::DietarySugar => "Dietary Sugar",
            Self::DietaryCaffeine => "Dietary Caffeine",
            Self::DietarySodium => "Dietary Sodium",
            Self::DietaryCholesterol => "Dietary Cholesterol",
            Self::DietaryIron => "Dietary Iron",
            Self::DietaryVitaminC => "Dietary Vitamin C",
            Self::DietaryVitaminD => "Dietary Vitamin D",
            Self::DietaryCalcium => "Dietary Calcium",
            Self::DietaryPotassium => "Dietary Potassium",
            Self::DietaryZinc => "Dietary Zinc",
            Self::DietaryMagnesium => "Dietary Magnesium",
            Self::TimeInDaylight => "Time in Daylight",
            Self::EnvironmentalAudio => "Environmental Audio",
            Self::HeadphoneAudio => "Headphone Audio",
            Self::MindfulSession => "Mindful Session",
            Self::HighHeartRateEvent => "High Heart Rate Event",
            Self::LowHeartRateEvent => "Low Heart Rate Event",
            Self::IrregularHeartRhythmEvent => "Irregular Heart Rhythm Event",
            Self::StandHour => "Stand Hour",
            Self::Falls => "Falls",
        }
    }

    /// Aggregation strategy for time-series bucketing.
    pub fn aggregation(&self) -> Aggregation {
        match self {
            // Avg — instantaneous or average measurements
            Self::HeartRate
            | Self::HeartRateVariability
            | Self::RestingHeartRate
            | Self::BloodPressureSystolic
            | Self::BloodPressureDiastolic
            | Self::BloodGlucose
            | Self::BloodOxygen
            | Self::RespiratoryRate
            | Self::Vo2Max
            | Self::WalkingHeartRate
            | Self::SleepingWristTemperature
            | Self::BodyMass
            | Self::BodyFatPercentage
            | Self::BodyTemperature
            | Self::LeanBodyMass
            | Self::Height
            | Self::WaistCircumference
            | Self::Bmi
            | Self::PhysicalEffort
            | Self::RunningSpeed
            | Self::RunningPower
            | Self::RunningStrideLength
            | Self::RunningVerticalOscillation
            | Self::RunningGroundContactTime
            | Self::CyclingSpeed
            | Self::CyclingPower
            | Self::CyclingCadence
            | Self::CyclingFtp
            | Self::WalkingSpeed
            | Self::WalkingDoubleSupport
            | Self::WalkingStepLength
            | Self::WalkingAsymmetry
            | Self::StairAscentSpeed
            | Self::StairDescentSpeed
            | Self::SixMinWalkDistance
            | Self::EnvironmentalAudio
            | Self::HeadphoneAudio => Aggregation::Avg,

            // Sum — cumulative metrics
            Self::Steps
            | Self::ActiveEnergy
            | Self::BasalEnergy
            | Self::DistanceWalkingRunning
            | Self::FlightsClimbed
            | Self::ExerciseTime
            | Self::StandTime
            | Self::DistanceCycling
            | Self::DistanceSwimming
            | Self::SwimmingStrokes
            | Self::MoveTime
            | Self::InsulinDelivery
            | Self::WaterIntake
            | Self::DietaryEnergy
            | Self::DietaryProtein
            | Self::DietaryFat
            | Self::DietaryCarbs
            | Self::DietaryFiber
            | Self::DietarySugar
            | Self::DietaryCaffeine
            | Self::DietarySodium
            | Self::DietaryCholesterol
            | Self::DietaryIron
            | Self::DietaryVitaminC
            | Self::DietaryVitaminD
            | Self::DietaryCalcium
            | Self::DietaryPotassium
            | Self::DietaryZinc
            | Self::DietaryMagnesium
            | Self::TimeInDaylight
            | Self::Falls => Aggregation::Sum,

            // SleepDuration — sum segment durations where value IN (1,3,4,5)
            Self::SleepAnalysis | Self::MindfulSession => Aggregation::SleepDuration,

            // CountEvents — count occurrences per bucket
            Self::HighHeartRateEvent
            | Self::LowHeartRateEvent
            | Self::IrregularHeartRhythmEvent
            | Self::StandHour => Aggregation::CountEvents,
        }
    }

    /// Category for grouping in the metrics picker.
    pub fn category(&self) -> &'static str {
        match self {
            Self::HeartRate
            | Self::HeartRateVariability
            | Self::RestingHeartRate
            | Self::BloodPressureSystolic
            | Self::BloodPressureDiastolic
            | Self::BloodGlucose
            | Self::BloodOxygen
            | Self::RespiratoryRate
            | Self::Vo2Max
            | Self::WalkingHeartRate
            | Self::InsulinDelivery
            | Self::SleepingWristTemperature => "Vitals",

            Self::BodyMass
            | Self::BodyFatPercentage
            | Self::BodyTemperature
            | Self::LeanBodyMass
            | Self::Height
            | Self::WaistCircumference
            | Self::Bmi => "Body",

            Self::Steps
            | Self::ActiveEnergy
            | Self::BasalEnergy
            | Self::DistanceWalkingRunning
            | Self::FlightsClimbed
            | Self::ExerciseTime
            | Self::StandTime
            | Self::DistanceCycling
            | Self::DistanceSwimming
            | Self::SwimmingStrokes
            | Self::MoveTime
            | Self::PhysicalEffort => "Activity",

            Self::RunningSpeed
            | Self::RunningPower
            | Self::RunningStrideLength
            | Self::RunningVerticalOscillation
            | Self::RunningGroundContactTime => "Running",

            Self::CyclingSpeed | Self::CyclingPower | Self::CyclingCadence | Self::CyclingFtp => {
                "Cycling"
            }

            Self::WalkingSpeed
            | Self::WalkingDoubleSupport
            | Self::WalkingStepLength
            | Self::WalkingAsymmetry
            | Self::StairAscentSpeed
            | Self::StairDescentSpeed
            | Self::SixMinWalkDistance => "Mobility",

            Self::SleepAnalysis => "Sleep",

            Self::WaterIntake
            | Self::DietaryEnergy
            | Self::DietaryProtein
            | Self::DietaryFat
            | Self::DietaryCarbs
            | Self::DietaryFiber
            | Self::DietarySugar
            | Self::DietaryCaffeine
            | Self::DietarySodium
            | Self::DietaryCholesterol
            | Self::DietaryIron
            | Self::DietaryVitaminC
            | Self::DietaryVitaminD
            | Self::DietaryCalcium
            | Self::DietaryPotassium
            | Self::DietaryZinc
            | Self::DietaryMagnesium => "Dietary",

            Self::TimeInDaylight | Self::EnvironmentalAudio | Self::HeadphoneAudio => "Environment",

            Self::MindfulSession
            | Self::HighHeartRateEvent
            | Self::LowHeartRateEvent
            | Self::IrregularHeartRhythmEvent
            | Self::StandHour
            | Self::Falls => "Events",
        }
    }

    /// All variants, for building the static metric list.
    pub fn all() -> &'static [Self] {
        &[
            // Vitals
            Self::HeartRate,
            Self::HeartRateVariability,
            Self::RestingHeartRate,
            Self::BloodPressureSystolic,
            Self::BloodPressureDiastolic,
            Self::BloodGlucose,
            Self::BloodOxygen,
            Self::RespiratoryRate,
            Self::Vo2Max,
            Self::WalkingHeartRate,
            Self::InsulinDelivery,
            Self::SleepingWristTemperature,
            // Body
            Self::BodyMass,
            Self::BodyFatPercentage,
            Self::BodyTemperature,
            Self::LeanBodyMass,
            Self::Height,
            Self::WaistCircumference,
            Self::Bmi,
            // Activity
            Self::Steps,
            Self::ActiveEnergy,
            Self::BasalEnergy,
            Self::DistanceWalkingRunning,
            Self::FlightsClimbed,
            Self::ExerciseTime,
            Self::StandTime,
            Self::DistanceCycling,
            Self::DistanceSwimming,
            Self::SwimmingStrokes,
            Self::MoveTime,
            Self::PhysicalEffort,
            // Running
            Self::RunningSpeed,
            Self::RunningPower,
            Self::RunningStrideLength,
            Self::RunningVerticalOscillation,
            Self::RunningGroundContactTime,
            // Cycling
            Self::CyclingSpeed,
            Self::CyclingPower,
            Self::CyclingCadence,
            Self::CyclingFtp,
            // Mobility
            Self::WalkingSpeed,
            Self::WalkingDoubleSupport,
            Self::WalkingStepLength,
            Self::WalkingAsymmetry,
            Self::StairAscentSpeed,
            Self::StairDescentSpeed,
            Self::SixMinWalkDistance,
            // Sleep
            Self::SleepAnalysis,
            // Dietary
            Self::WaterIntake,
            Self::DietaryEnergy,
            Self::DietaryProtein,
            Self::DietaryFat,
            Self::DietaryCarbs,
            Self::DietaryFiber,
            Self::DietarySugar,
            Self::DietaryCaffeine,
            Self::DietarySodium,
            Self::DietaryCholesterol,
            Self::DietaryIron,
            Self::DietaryVitaminC,
            Self::DietaryVitaminD,
            Self::DietaryCalcium,
            Self::DietaryPotassium,
            Self::DietaryZinc,
            Self::DietaryMagnesium,
            // Environment
            Self::TimeInDaylight,
            Self::EnvironmentalAudio,
            Self::HeadphoneAudio,
            // Events
            Self::MindfulSession,
            Self::HighHeartRateEvent,
            Self::LowHeartRateEvent,
            Self::IrregularHeartRhythmEvent,
            Self::StandHour,
            Self::Falls,
        ]
    }
}

impl CheckinField {
    /// Column name in the `daily_checkins` table.
    pub fn column(&self) -> &'static str {
        match self {
            Self::Energy => "energy",
            Self::Mood => "mood",
            Self::Focus => "focus",
            Self::Recovery => "recovery",
            Self::Libido => "libido",
        }
    }

    pub fn parse(field: &str) -> Option<Self> {
        match field {
            "energy" => Some(Self::Energy),
            "mood" => Some(Self::Mood),
            "focus" => Some(Self::Focus),
            "recovery" => Some(Self::Recovery),
            "libido" => Some(Self::Libido),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Energy => "Energy",
            Self::Mood => "Mood",
            Self::Focus => "Focus",
            Self::Recovery => "Recovery",
            Self::Libido => "Libido",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Energy,
            Self::Mood,
            Self::Focus,
            Self::Recovery,
            Self::Libido,
        ]
    }
}

impl CalendarField {
    pub fn column(&self) -> &'static str {
        match self {
            Self::MeetingMinutes => "meeting_minutes",
            Self::MeetingCount => "meeting_count",
        }
    }

    pub fn parse(field: &str) -> Option<Self> {
        match field {
            "meeting_minutes" => Some(Self::MeetingMinutes),
            "meeting_count" => Some(Self::MeetingCount),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::MeetingMinutes => "Meeting Minutes",
            Self::MeetingCount => "Meeting Count",
        }
    }

    pub fn unit(&self) -> &'static str {
        match self {
            Self::MeetingMinutes => "min",
            Self::MeetingCount => "count",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::MeetingMinutes, Self::MeetingCount]
    }
}

impl SleepField {
    /// JSONB key in `observations.value` for sleep records.
    pub fn json_key(&self) -> &'static str {
        match self {
            Self::DurationMinutes => "duration_minutes",
            Self::DeepMinutes => "deep_minutes",
            Self::RemMinutes => "rem_minutes",
            Self::Score => "score",
        }
    }

    pub fn parse(field: &str) -> Option<Self> {
        match field {
            "duration_minutes" => Some(Self::DurationMinutes),
            "deep_minutes" => Some(Self::DeepMinutes),
            "rem_minutes" => Some(Self::RemMinutes),
            "score" => Some(Self::Score),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::DurationMinutes => "Sleep Duration",
            Self::DeepMinutes => "Deep Sleep",
            Self::RemMinutes => "REM Sleep",
            Self::Score => "Sleep Score",
        }
    }

    pub fn unit(&self) -> &'static str {
        match self {
            Self::DurationMinutes | Self::DeepMinutes | Self::RemMinutes => "min",
            Self::Score => "score",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::DurationMinutes,
            Self::DeepMinutes,
            Self::RemMinutes,
            Self::Score,
        ]
    }
}

impl MetricSource {
    /// Parse a (source, field) pair into a validated `MetricSource`.
    /// Returns `ApiError::BadRequest` if the source or field is not in the allowlist.
    pub fn parse(source: &str, field: &str) -> Result<Self, ApiError> {
        match source {
            "health_records" => HealthRecordField::parse(field)
                .map(MetricSource::HealthRecord)
                .ok_or_else(|| {
                    ApiError::BadRequest(format!("invalid health_records field: {field}"))
                }),
            "checkins" => CheckinField::parse(field)
                .map(MetricSource::Checkin)
                .ok_or_else(|| ApiError::BadRequest(format!("invalid checkins field: {field}"))),
            "labs" => {
                if field.is_empty() {
                    Err(ApiError::BadRequest(
                        "labs field (marker name) must not be empty".to_string(),
                    ))
                } else {
                    Ok(MetricSource::Lab(field.to_string()))
                }
            }
            "calendar" => CalendarField::parse(field)
                .map(MetricSource::Calendar)
                .ok_or_else(|| ApiError::BadRequest(format!("invalid calendar field: {field}"))),
            "sleep" => SleepField::parse(field)
                .map(MetricSource::Sleep)
                .ok_or_else(|| ApiError::BadRequest(format!("invalid sleep field: {field}"))),
            "observer_polls" => {
                // Field format: "<poll_id>:<dimension>"
                let (poll_id_str, dimension) = field.split_once(':').ok_or_else(|| {
                    ApiError::BadRequest(
                        "observer_polls field must be <poll_id>:<dimension>".to_string(),
                    )
                })?;
                let poll_id = Uuid::parse_str(poll_id_str).map_err(|_| {
                    ApiError::BadRequest(format!("invalid poll_id in field: {poll_id_str}"))
                })?;
                if dimension.is_empty() {
                    return Err(ApiError::BadRequest(
                        "observer_polls dimension must not be empty".to_string(),
                    ));
                }
                // Safety: dimension is interpolated into a JSONB key selector
                // via format!. Restrict to alphanumeric + underscore to prevent
                // any SQL injection through the key name.
                if !dimension
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    return Err(ApiError::BadRequest(
                        "observer_polls dimension must contain only alphanumeric characters and underscores".to_string(),
                    ));
                }
                Ok(MetricSource::ObserverPoll(ObserverPollField {
                    poll_id,
                    dimension: dimension.to_string(),
                }))
            }
            _ => Err(ApiError::BadRequest(format!("invalid source: {source}"))),
        }
    }

    /// Return the unit string for this metric.
    pub fn unit(&self) -> String {
        match self {
            MetricSource::HealthRecord(f) => f.unit().to_string(),
            MetricSource::Checkin(_) => "score".to_string(),
            MetricSource::Lab(_) => "value".to_string(),
            MetricSource::Calendar(f) => f.unit().to_string(),
            MetricSource::Sleep(f) => f.unit().to_string(),
            MetricSource::ObserverPoll(_) => "score".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct SeriesResponse {
    pub source: String,
    pub field: String,
    pub unit: String,
    pub points: Vec<DataPoint>,
}

#[derive(Debug, Serialize)]
pub struct DataPoint {
    pub t: DateTime<Utc>,
    pub v: f64,
    pub n: i64,
}

#[derive(Debug, Deserialize)]
pub struct SeriesQuery {
    pub source: String,
    pub field: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub resolution: Resolution,
}

#[derive(Debug, Deserialize)]
pub struct BatchSeriesRequest {
    pub metrics: Vec<MetricSpec>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub resolution: Resolution,
}

#[derive(Debug, Deserialize)]
pub struct MetricSpec {
    pub source: String,
    pub field: String,
}

#[derive(Debug, Serialize)]
pub struct MetricsResponse {
    pub sources: Vec<MetricSourceGroup>,
}

#[derive(Debug, Serialize)]
pub struct MetricSourceGroup {
    pub source: String,
    pub label: String,
    pub metrics: Vec<MetricOption>,
}

#[derive(Debug, Serialize)]
pub struct MetricOption {
    pub field: String,
    pub label: String,
    pub unit: String,
}

// ---------------------------------------------------------------------------
// Saved chart types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateChart {
    pub name: String,
    pub config: ChartConfig,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChart {
    pub name: Option<String>,
    pub config: Option<ChartConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChartConfig {
    pub version: u8,
    pub metrics: Vec<ChartMetricConfig>,
    pub range: ChartRange,
    pub resolution: Resolution,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChartMetricConfig {
    pub source: String,
    pub field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ChartRange {
    Preset { preset: String },
    Custom { start: String, end: String },
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ChartRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub config: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Validate chart config: version, metric count, metric fields, range, colors.
pub fn validate_chart_config(config: &ChartConfig) -> Result<(), ApiError> {
    if config.version != 1 {
        return Err(ApiError::BadRequest(format!(
            "unsupported chart config version: {}",
            config.version
        )));
    }

    if config.metrics.is_empty() {
        return Err(ApiError::BadRequest(
            "chart must have at least one metric".to_string(),
        ));
    }

    if config.metrics.len() > 8 {
        return Err(ApiError::BadRequest(
            "chart may have at most 8 metrics".to_string(),
        ));
    }

    for m in &config.metrics {
        MetricSource::parse(&m.source, &m.field)?;

        if let Some(ref color) = m.color
            && !is_valid_hex_color(color)
        {
            return Err(ApiError::BadRequest(format!(
                "invalid color: {color} (expected #rrggbb)"
            )));
        }
    }

    match &config.range {
        ChartRange::Preset { preset } => {
            if !["7d", "30d", "90d", "1y", "all"].contains(&preset.as_str()) {
                return Err(ApiError::BadRequest(format!(
                    "invalid range preset: {preset}"
                )));
            }
        }
        ChartRange::Custom { start, end } => {
            // Validate that start and end are parseable dates
            chrono::NaiveDate::parse_from_str(start, "%Y-%m-%d").map_err(|_| {
                ApiError::BadRequest(format!("invalid custom range start date: {start}"))
            })?;
            chrono::NaiveDate::parse_from_str(end, "%Y-%m-%d").map_err(|_| {
                ApiError::BadRequest(format!("invalid custom range end date: {end}"))
            })?;
        }
    }

    Ok(())
}

fn is_valid_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

// ---------------------------------------------------------------------------
// Intervention marker types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct InterventionMarkersQuery {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct InterventionMarker {
    pub t: DateTime<Utc>,
    pub substance: String,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
}

// ---------------------------------------------------------------------------
// SSE event types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct DataChangedEvent {
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_health_record_fields() {
        for field in HealthRecordField::all() {
            let result = MetricSource::parse("health_records", field.record_type());
            assert!(result.is_ok(), "failed for {:?}", field);
        }
    }

    #[test]
    fn parse_valid_checkin_fields() {
        for field in CheckinField::all() {
            let result = MetricSource::parse("checkins", field.column());
            assert!(result.is_ok(), "failed for {:?}", field);
        }
    }

    #[test]
    fn parse_valid_calendar_fields() {
        for field in CalendarField::all() {
            let result = MetricSource::parse("calendar", field.column());
            assert!(result.is_ok(), "failed for {:?}", field);
        }
    }

    #[test]
    fn parse_valid_sleep_fields() {
        for field in SleepField::all() {
            let result = MetricSource::parse("sleep", field.json_key());
            assert!(result.is_ok(), "failed for {:?}", field);
        }
    }

    #[test]
    fn parse_valid_lab_field() {
        let result = MetricSource::parse("labs", "testosterone");
        assert!(result.is_ok());
    }

    #[test]
    fn parse_invalid_source() {
        let result = MetricSource::parse("invalid_source", "heart_rate");
        assert!(result.is_err());
    }

    #[test]
    fn parse_invalid_health_record_field() {
        let result = MetricSource::parse("health_records", "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn parse_invalid_checkin_field() {
        let result = MetricSource::parse("checkins", "happiness");
        assert!(result.is_err());
    }

    #[test]
    fn parse_empty_lab_marker() {
        let result = MetricSource::parse("labs", "");
        assert!(result.is_err());
    }

    #[test]
    fn resolution_serde_roundtrip() {
        let json = serde_json::to_string(&Resolution::Daily).unwrap();
        assert_eq!(json, r#""daily""#);
        let parsed: Resolution = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Resolution::Daily);

        let json = serde_json::to_string(&Resolution::Weekly).unwrap();
        assert_eq!(json, r#""weekly""#);
        let parsed: Resolution = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Resolution::Weekly);

        let json = serde_json::to_string(&Resolution::Monthly).unwrap();
        assert_eq!(json, r#""monthly""#);
        let parsed: Resolution = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Resolution::Monthly);
    }

    #[test]
    fn resolution_pg_interval() {
        assert_eq!(Resolution::Daily.pg_interval(), "day");
        assert_eq!(Resolution::Weekly.pg_interval(), "week");
        assert_eq!(Resolution::Monthly.pg_interval(), "month");
    }

    #[test]
    fn valid_chart_config() {
        let config = ChartConfig {
            version: 1,
            metrics: vec![ChartMetricConfig {
                source: "checkins".to_string(),
                field: "energy".to_string(),
                color: Some("#ff0000".to_string()),
            }],
            range: ChartRange::Preset {
                preset: "30d".to_string(),
            },
            resolution: Resolution::Daily,
        };
        assert!(validate_chart_config(&config).is_ok());
    }

    #[test]
    fn invalid_chart_version() {
        let config = ChartConfig {
            version: 2,
            metrics: vec![ChartMetricConfig {
                source: "checkins".to_string(),
                field: "energy".to_string(),
                color: None,
            }],
            range: ChartRange::Preset {
                preset: "30d".to_string(),
            },
            resolution: Resolution::Daily,
        };
        assert!(validate_chart_config(&config).is_err());
    }

    #[test]
    fn invalid_chart_empty_metrics() {
        let config = ChartConfig {
            version: 1,
            metrics: vec![],
            range: ChartRange::Preset {
                preset: "30d".to_string(),
            },
            resolution: Resolution::Daily,
        };
        assert!(validate_chart_config(&config).is_err());
    }

    #[test]
    fn invalid_chart_too_many_metrics() {
        let metrics: Vec<ChartMetricConfig> = (0..9)
            .map(|_| ChartMetricConfig {
                source: "checkins".to_string(),
                field: "energy".to_string(),
                color: None,
            })
            .collect();
        let config = ChartConfig {
            version: 1,
            metrics,
            range: ChartRange::Preset {
                preset: "30d".to_string(),
            },
            resolution: Resolution::Daily,
        };
        assert!(validate_chart_config(&config).is_err());
    }

    #[test]
    fn invalid_chart_bad_preset() {
        let config = ChartConfig {
            version: 1,
            metrics: vec![ChartMetricConfig {
                source: "checkins".to_string(),
                field: "energy".to_string(),
                color: None,
            }],
            range: ChartRange::Preset {
                preset: "5d".to_string(),
            },
            resolution: Resolution::Daily,
        };
        assert!(validate_chart_config(&config).is_err());
    }

    #[test]
    fn invalid_chart_bad_color() {
        let config = ChartConfig {
            version: 1,
            metrics: vec![ChartMetricConfig {
                source: "checkins".to_string(),
                field: "energy".to_string(),
                color: Some("red".to_string()),
            }],
            range: ChartRange::Preset {
                preset: "30d".to_string(),
            },
            resolution: Resolution::Daily,
        };
        assert!(validate_chart_config(&config).is_err());
    }

    #[test]
    fn valid_chart_custom_range() {
        let config = ChartConfig {
            version: 1,
            metrics: vec![ChartMetricConfig {
                source: "health_records".to_string(),
                field: "heart_rate".to_string(),
                color: None,
            }],
            range: ChartRange::Custom {
                start: "2026-01-01".to_string(),
                end: "2026-03-01".to_string(),
            },
            resolution: Resolution::Weekly,
        };
        assert!(validate_chart_config(&config).is_ok());
    }

    #[test]
    fn invalid_chart_bad_custom_date() {
        let config = ChartConfig {
            version: 1,
            metrics: vec![ChartMetricConfig {
                source: "checkins".to_string(),
                field: "energy".to_string(),
                color: None,
            }],
            range: ChartRange::Custom {
                start: "not-a-date".to_string(),
                end: "2026-03-01".to_string(),
            },
            resolution: Resolution::Daily,
        };
        assert!(validate_chart_config(&config).is_err());
    }

    #[test]
    fn parse_valid_observer_polls_field() {
        let poll_id = Uuid::new_v4();
        let field = format!("{poll_id}:energy");
        let result = MetricSource::parse("observer_polls", &field);
        assert!(result.is_ok());
        if let MetricSource::ObserverPoll(f) = result.unwrap() {
            assert_eq!(f.poll_id, poll_id);
            assert_eq!(f.dimension, "energy");
        } else {
            panic!("expected ObserverPoll variant");
        }
    }

    #[test]
    fn parse_observer_polls_invalid_format() {
        let result = MetricSource::parse("observer_polls", "not-a-uuid-colon-dim");
        assert!(result.is_err());
    }

    #[test]
    fn parse_observer_polls_missing_dimension() {
        let poll_id = Uuid::new_v4();
        let field = format!("{poll_id}:");
        let result = MetricSource::parse("observer_polls", &field);
        assert!(result.is_err());
    }

    #[test]
    fn parse_observer_polls_no_colon() {
        let result = MetricSource::parse("observer_polls", "just-a-string");
        assert!(result.is_err());
    }

    #[test]
    fn parse_observer_polls_rejects_injection() {
        let poll_id = Uuid::new_v4();
        // Attempt SQL injection via dimension name
        let field = format!("{poll_id}:energy' OR '1'='1");
        let result = MetricSource::parse("observer_polls", &field);
        assert!(result.is_err());
    }

    #[test]
    fn hex_color_validation() {
        assert!(is_valid_hex_color("#ff0000"));
        assert!(is_valid_hex_color("#AABBCC"));
        assert!(!is_valid_hex_color("ff0000"));
        assert!(!is_valid_hex_color("#fff"));
        assert!(!is_valid_hex_color("#gggggg"));
    }

    #[test]
    fn all_health_record_fields_parse_roundtrip() {
        for field in HealthRecordField::all() {
            let rt = field.record_type();
            let parsed = HealthRecordField::parse(rt);
            assert_eq!(parsed, Some(*field), "parse roundtrip failed for {rt}");
        }
    }

    #[test]
    fn all_health_record_fields_have_nonempty_label_and_unit() {
        for field in HealthRecordField::all() {
            assert!(!field.label().is_empty(), "empty label for {:?}", field);
            assert!(!field.unit().is_empty(), "empty unit for {:?}", field);
        }
    }

    #[test]
    fn all_health_record_fields_have_aggregation() {
        for field in HealthRecordField::all() {
            // Just ensure aggregation() does not panic
            let _ = field.aggregation();
        }
    }

    #[test]
    fn all_health_record_fields_have_category() {
        for field in HealthRecordField::all() {
            let cat = field.category();
            assert!(
                [
                    "Vitals",
                    "Body",
                    "Activity",
                    "Running",
                    "Cycling",
                    "Mobility",
                    "Sleep",
                    "Dietary",
                    "Environment",
                    "Events"
                ]
                .contains(&cat),
                "unexpected category {cat} for {:?}",
                field
            );
        }
    }

    #[test]
    fn aggregation_values_correct() {
        assert_eq!(HealthRecordField::HeartRate.aggregation(), Aggregation::Avg);
        assert_eq!(HealthRecordField::Steps.aggregation(), Aggregation::Sum);
        assert_eq!(
            HealthRecordField::SleepAnalysis.aggregation(),
            Aggregation::SleepDuration
        );
        assert_eq!(
            HealthRecordField::HighHeartRateEvent.aggregation(),
            Aggregation::CountEvents
        );
        assert_eq!(
            HealthRecordField::DietaryProtein.aggregation(),
            Aggregation::Sum
        );
        assert_eq!(
            HealthRecordField::RunningSpeed.aggregation(),
            Aggregation::Avg
        );
        assert_eq!(
            HealthRecordField::MindfulSession.aggregation(),
            Aggregation::SleepDuration
        );
        assert_eq!(HealthRecordField::Falls.aggregation(), Aggregation::Sum);
    }

    #[test]
    fn parse_new_health_record_fields() {
        // Spot-check a selection of new variants
        assert_eq!(
            HealthRecordField::parse("distance_walking_running"),
            Some(HealthRecordField::DistanceWalkingRunning)
        );
        assert_eq!(
            HealthRecordField::parse("sleep_analysis"),
            Some(HealthRecordField::SleepAnalysis)
        );
        assert_eq!(
            HealthRecordField::parse("dietary_protein"),
            Some(HealthRecordField::DietaryProtein)
        );
        assert_eq!(
            HealthRecordField::parse("cycling_ftp"),
            Some(HealthRecordField::CyclingFtp)
        );
        assert_eq!(
            HealthRecordField::parse("high_heart_rate_event"),
            Some(HealthRecordField::HighHeartRateEvent)
        );
        assert_eq!(
            HealthRecordField::parse("time_in_daylight"),
            Some(HealthRecordField::TimeInDaylight)
        );
    }

    #[test]
    fn health_record_field_count() {
        // Ensure all() has the expected number of variants (74)
        assert_eq!(HealthRecordField::all().len(), 74);
    }
}
