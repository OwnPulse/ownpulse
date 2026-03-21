// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Charts
import HealthKit
import SwiftUI

struct NightSummary: Identifiable {
    let id: Date // wake date (morning)
    let deepMinutes: Double
    let coreMinutes: Double
    let remMinutes: Double
    let awakeMinutes: Double
    let avgHRV: Double? // ms, nil if no HRV data
}

struct SleepChartView: View {
    @State private var nights: [NightSummary] = []
    @State private var isLoading = true
    @State private var error: String?

    private let store = HKHealthStore()

    var body: some View {
        GroupBox("Sleep & HRV") {
            if isLoading {
                ProgressView()
                    .frame(maxWidth: .infinity, minHeight: 200)
                    .accessibilityIdentifier("sleepChartLoading")
            } else if let error {
                Text(error)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, minHeight: 60)
                    .accessibilityIdentifier("sleepChartError")
            } else if nights.isEmpty {
                Text("No sleep data for the last 14 days.")
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, minHeight: 60)
                    .accessibilityIdentifier("sleepChartEmpty")
            } else {
                chartContent
                    .accessibilityIdentifier("sleepChart")
            }
        }
        .task {
            await loadData()
        }
    }

    @ViewBuilder
    private var chartContent: some View {
        let maxSleep = nights.map { $0.deepMinutes + $0.coreMinutes + $0.remMinutes + $0.awakeMinutes }.max() ?? 480

        Chart {
            ForEach(nights) { night in
                BarMark(
                    x: .value("Date", night.id, unit: .day),
                    y: .value("Minutes", night.deepMinutes)
                )
                .foregroundStyle(by: .value("Stage", "Deep"))

                BarMark(
                    x: .value("Date", night.id, unit: .day),
                    y: .value("Minutes", night.coreMinutes)
                )
                .foregroundStyle(by: .value("Stage", "Core"))

                BarMark(
                    x: .value("Date", night.id, unit: .day),
                    y: .value("Minutes", night.remMinutes)
                )
                .foregroundStyle(by: .value("Stage", "REM"))

                BarMark(
                    x: .value("Date", night.id, unit: .day),
                    y: .value("Minutes", night.awakeMinutes)
                )
                .foregroundStyle(by: .value("Stage", "Awake"))
            }

            // HRV line overlay — scaled to fit the sleep Y axis
            let hrvNights = nights.compactMap { n in n.avgHRV.map { (n.id, $0) } }
            if let hrvMin = hrvNights.map(\.1).min(),
               let hrvMax = hrvNights.map(\.1).max(),
               hrvMax > hrvMin {
                let range = hrvMax - hrvMin
                let padMin = hrvMin - range * 0.1
                let padMax = hrvMax + range * 0.1
                let padRange = padMax - padMin

                ForEach(hrvNights, id: \.0) { date, hrv in
                    let scaled = (hrv - padMin) / padRange * maxSleep
                    LineMark(
                        x: .value("Date", date, unit: .day),
                        y: .value("Minutes", scaled)
                    )
                    .foregroundStyle(.white)
                    .lineStyle(StrokeStyle(lineWidth: 2))
                    .interpolationMethod(.catmullRom)

                    PointMark(
                        x: .value("Date", date, unit: .day),
                        y: .value("Minutes", scaled)
                    )
                    .foregroundStyle(.white)
                    .symbolSize(20)
                }
            }
        }
        .chartForegroundStyleScale([
            "Deep": Color(hex: 0x1A365D),
            "Core": Color(hex: 0x63B3ED),
            "REM": Color(hex: 0x805AD5),
            "Awake": Color(hex: 0xED8936),
        ])
        .chartYAxis {
            AxisMarks(position: .leading) { value in
                AxisGridLine()
                AxisValueLabel {
                    if let mins = value.as(Double.self) {
                        Text("\(Int(mins))m")
                    }
                }
            }
        }
        .chartXAxis {
            AxisMarks(values: .stride(by: .day, count: 2)) { value in
                AxisValueLabel(format: .dateTime.month(.abbreviated).day())
            }
        }
        .frame(height: 260)

        // HRV legend
        if let minHRV = nights.compactMap(\.avgHRV).min(),
           let maxHRV = nights.compactMap(\.avgHRV).max() {
            HStack {
                Circle().fill(.white).frame(width: 8, height: 8)
                    .overlay(Circle().stroke(.secondary, lineWidth: 1))
                Text("HRV: \(Int(minHRV))–\(Int(maxHRV)) ms")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .accessibilityIdentifier("sleepChartHRVLegend")
        }
    }

    private func loadData() async {
        guard HKHealthStore.isHealthDataAvailable() else {
            error = "HealthKit not available"
            isLoading = false
            return
        }

        let sleepType = HKCategoryType(.sleepAnalysis)
        let hrvType = HKQuantityType(.heartRateVariabilitySDNN)

        do {
            try await store.requestAuthorization(toShare: [], read: [sleepType, hrvType])
        } catch {
            self.error = "HealthKit access denied"
            isLoading = false
            return
        }

        let calendar = Calendar.current
        let now = Date()
        let startDate = calendar.date(byAdding: .day, value: -15, to: now)!

        async let sleepSamples = querySleep(type: sleepType, from: startDate, to: now)
        async let hrvSamples = queryHRV(type: hrvType, from: startDate, to: now)

        let sleep = (try? await sleepSamples) ?? []
        let hrv = (try? await hrvSamples) ?? []

        // Group sleep by night (wake date: samples before 18:00 → that day, after → next day)
        var nightBuckets: [Date: (deep: Double, core: Double, rem: Double, awake: Double)] = [:]

        for sample in sleep {
            let wakeDate = nightDate(for: sample.startDate, calendar: calendar)
            var bucket = nightBuckets[wakeDate, default: (0, 0, 0, 0)]
            let minutes = sample.endDate.timeIntervalSince(sample.startDate) / 60

            switch sample.value {
            case HKCategoryValueSleepAnalysis.asleepDeep.rawValue:
                bucket.deep += minutes
            case HKCategoryValueSleepAnalysis.asleepCore.rawValue,
                 HKCategoryValueSleepAnalysis.asleepUnspecified.rawValue:
                bucket.core += minutes
            case HKCategoryValueSleepAnalysis.asleepREM.rawValue:
                bucket.rem += minutes
            case HKCategoryValueSleepAnalysis.awake.rawValue:
                bucket.awake += minutes
            default: break // inBed, etc
            }
            nightBuckets[wakeDate] = bucket
        }

        // Group HRV by night
        var hrvBuckets: [Date: [Double]] = [:]
        for sample in hrv {
            let wakeDate = nightDate(for: sample.startDate, calendar: calendar)
            hrvBuckets[wakeDate, default: []].append(
                sample.quantity.doubleValue(for: HKUnit.secondUnit(with: .milli))
            )
        }

        // Build summaries for last 14 days
        let cutoff = calendar.date(byAdding: .day, value: -14, to: now)!
        nights = nightBuckets
            .filter { $0.key >= cutoff }
            .sorted { $0.key < $1.key }
            .map { date, bucket in
                let hrvValues = hrvBuckets[date]
                let avgHRV = hrvValues.flatMap { vals in
                    vals.isEmpty ? nil : vals.reduce(0, +) / Double(vals.count)
                }
                return NightSummary(
                    id: date,
                    deepMinutes: bucket.deep,
                    coreMinutes: bucket.core,
                    remMinutes: bucket.rem,
                    awakeMinutes: bucket.awake,
                    avgHRV: avgHRV
                )
            }

        isLoading = false
    }

    private func nightDate(for date: Date, calendar: Calendar) -> Date {
        let hour = calendar.component(.hour, from: date)
        let day = calendar.startOfDay(for: date)
        // Before 6pm → belongs to this day's morning; after 6pm → next day's morning
        return hour < 18 ? day : calendar.date(byAdding: .day, value: 1, to: day)!
    }

    private func querySleep(type: HKCategoryType, from: Date, to: Date) async throws -> [HKCategorySample] {
        try await withCheckedThrowingContinuation { continuation in
            let predicate = HKQuery.predicateForSamples(withStart: from, end: to)
            let query = HKSampleQuery(
                sampleType: type,
                predicate: predicate,
                limit: HKObjectQueryNoLimit,
                sortDescriptors: [NSSortDescriptor(key: HKSampleSortIdentifierStartDate, ascending: true)]
            ) { _, results, error in
                if let error { continuation.resume(throwing: error) }
                else { continuation.resume(returning: (results as? [HKCategorySample]) ?? []) }
            }
            store.execute(query)
        }
    }

    private func queryHRV(type: HKQuantityType, from: Date, to: Date) async throws -> [HKQuantitySample] {
        try await withCheckedThrowingContinuation { continuation in
            let predicate = HKQuery.predicateForSamples(withStart: from, end: to)
            let query = HKSampleQuery(
                sampleType: type,
                predicate: predicate,
                limit: HKObjectQueryNoLimit,
                sortDescriptors: [NSSortDescriptor(key: HKSampleSortIdentifierStartDate, ascending: true)]
            ) { _, results, error in
                if let error { continuation.resume(throwing: error) }
                else { continuation.resume(returning: (results as? [HKQuantitySample]) ?? []) }
            }
            store.execute(query)
        }
    }
}

private extension Color {
    init(hex: UInt32) {
        let r = Double((hex >> 16) & 0xFF) / 255
        let g = Double((hex >> 8) & 0xFF) / 255
        let b = Double(hex & 0xFF) / 255
        self.init(red: r, green: g, blue: b)
    }
}
