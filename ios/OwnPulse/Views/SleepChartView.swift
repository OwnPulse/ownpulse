// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI
import Charts
import HealthKit

// MARK: - Data models

struct SleepNight: Identifiable {
    /// The calendar date on which the person woke up (the "morning" end of the night).
    let id: Date          // wake date, midnight
    let deepMinutes: Double
    let coreMinutes: Double
    let remMinutes: Double
    let awakeMinutes: Double
    /// Average HRV across samples recorded that night; nil when no data.
    let avgHRV: Double?

    var totalSleepMinutes: Double {
        deepMinutes + coreMinutes + remMinutes
    }
}

private enum SleepStage: String, CaseIterable, Plottable {
    case deep   = "Deep"
    case core   = "Core"
    case rem    = "REM"
    case awake  = "Awake"

    var color: Color {
        switch self {
        case .deep:  Color(red: 0x1a / 255.0, green: 0x36 / 255.0, blue: 0x5d / 255.0)
        case .core:  Color(red: 0x63 / 255.0, green: 0xb3 / 255.0, blue: 0xed / 255.0)
        case .rem:   Color(red: 0x80 / 255.0, green: 0x5a / 255.0, blue: 0xd5 / 255.0)
        case .awake: Color(red: 0xed / 255.0, green: 0x89 / 255.0, blue: 0x36 / 255.0)
        }
    }
}

// Flat row used by the stacked BarMark.
private struct SleepBar: Identifiable {
    let id = UUID()
    let night: Date
    let stage: SleepStage
    let minutes: Double
}

// MARK: - HealthKit query helpers

private actor SleepChartLoader {
    private let store = HKHealthStore()

    // Returns the 14 most-recent nights (wake date ascending), oldest first.
    func load() async throws -> [SleepNight] {
        let calendar = Calendar.current

        // --- Authorization check (incremental: only ask for what we need here) ---
        guard HKHealthStore.isHealthDataAvailable() else { return [] }

        let sleepType  = HKCategoryType(.sleepAnalysis)
        let hrvType    = HKQuantityType(.heartRateVariabilitySDNN)

        try await store.requestAuthorization(toShare: [], read: [sleepType, hrvType])

        // --- Date range: 14 nights back from last midnight ---
        let todayMidnight = calendar.startOfDay(for: Date())
        // Start = 6 pm 15 days ago (covers the leading edge of night 14).
        guard let windowStart = calendar.date(
            byAdding: .day, value: -15, to: todayMidnight
        ) else { return [] }

        // --- Query sleep samples ---
        let sleepPredicate = HKQuery.predicateForSamples(
            withStart: windowStart,
            end: Date(),
            options: .strictStartDate
        )

        let sleepSamples: [HKCategorySample] = try await withCheckedThrowingContinuation { cont in
            let query = HKSampleQuery(
                sampleType: sleepType,
                predicate: sleepPredicate,
                limit: HKObjectQueryNoLimit,
                sortDescriptors: [NSSortDescriptor(key: HKSampleSortIdentifierStartDate, ascending: true)]
            ) { _, samples, error in
                if let error {
                    cont.resume(throwing: error)
                } else {
                    cont.resume(returning: (samples as? [HKCategorySample]) ?? [])
                }
            }
            store.execute(query)
        }

        // --- Query HRV samples ---
        let hrvPredicate = HKQuery.predicateForSamples(
            withStart: windowStart,
            end: Date(),
            options: .strictStartDate
        )

        let hrvSamples: [HKQuantitySample] = try await withCheckedThrowingContinuation { cont in
            let query = HKSampleQuery(
                sampleType: hrvType,
                predicate: hrvPredicate,
                limit: HKObjectQueryNoLimit,
                sortDescriptors: [NSSortDescriptor(key: HKSampleSortIdentifierStartDate, ascending: true)]
            ) { _, samples, error in
                if let error {
                    cont.resume(throwing: error)
                } else {
                    cont.resume(returning: (samples as? [HKQuantitySample]) ?? [])
                }
            }
            store.execute(query)
        }

        // --- Group by night (6 pm – 6 pm window) ---
        // For a given sample, the "wake date" is the calendar day whose 6 pm
        // opening boundary is <= sample.startDate < next 6 pm.
        func wakeDate(for date: Date) -> Date {
            // Boundary: 6 pm on the previous calendar day.
            var components = calendar.dateComponents([.year, .month, .day], from: date)
            let hour = calendar.component(.hour, from: date)
            // If the sample is before 6 pm local time it belongs to a night that
            // started the previous evening, so we attribute it to today's wake date.
            // If it's 6 pm or later it belongs to a night that ends the following
            // morning, so wake date = tomorrow.
            if hour < 18 {
                // wake date = this calendar day's midnight
                return calendar.date(from: components)!
            } else {
                // wake date = next calendar day's midnight
                components.day! += 1
                return calendar.date(from: components)!
            }
        }

        // Build dictionaries keyed by wake-date midnight.
        var deepByNight:  [Date: Double] = [:]
        var coreByNight:  [Date: Double] = [:]
        var remByNight:   [Date: Double] = [:]
        var awakeByNight: [Date: Double] = [:]

        for sample in sleepSamples {
            let night = wakeDate(for: sample.startDate)
            let minutes = sample.endDate.timeIntervalSince(sample.startDate) / 60.0

            // HKCategoryValueSleepAnalysis raw values:
            //  0 = inBed, 1 = asleepUnspecified, 2 = awake,
            //  3 = asleepCore, 4 = asleepDeep, 5 = asleepREM
            switch sample.value {
            case HKCategoryValueSleepAnalysis.asleepDeep.rawValue:
                deepByNight[night, default: 0] += minutes
            case HKCategoryValueSleepAnalysis.asleepCore.rawValue:
                coreByNight[night, default: 0] += minutes
            case HKCategoryValueSleepAnalysis.asleepREM.rawValue:
                remByNight[night, default: 0] += minutes
            case HKCategoryValueSleepAnalysis.awake.rawValue:
                awakeByNight[night, default: 0] += minutes
            case HKCategoryValueSleepAnalysis.asleepUnspecified.rawValue:
                // Treat unspecified sleep as core.
                coreByNight[night, default: 0] += minutes
            default:
                break // inBed — ignore
            }
        }

        // Group HRV by wake night.
        var hrvValuesByNight: [Date: [Double]] = [:]
        let msUnit = HKUnit.secondUnit(with: .milli)
        for sample in hrvSamples {
            let night = wakeDate(for: sample.startDate)
            let ms = sample.quantity.doubleValue(for: msUnit)
            hrvValuesByNight[night, default: []].append(ms)
        }

        // Collect the 14 wake dates that fall within our window.
        var nights: [SleepNight] = []
        for offset in stride(from: -13, through: 0, by: 1) {
            guard let wakeDay = calendar.date(byAdding: .day, value: offset, to: todayMidnight) else { continue }

            // Only include nights that have at least some sleep data.
            let deep  = deepByNight[wakeDay]  ?? 0
            let core  = coreByNight[wakeDay]  ?? 0
            let rem   = remByNight[wakeDay]   ?? 0
            let awake = awakeByNight[wakeDay] ?? 0
            guard deep + core + rem + awake > 0 else { continue }

            let hrvValues = hrvValuesByNight[wakeDay]
            let avgHRV: Double? = hrvValues.map({ !$0.isNaN ? $0 : 0 }).isEmpty
                ? nil
                : hrvValues.reduce(0, +) / Double(hrvValues.count)

            nights.append(SleepNight(
                id: wakeDay,
                deepMinutes: deep,
                coreMinutes: core,
                remMinutes: rem,
                awakeMinutes: awake,
                avgHRV: avgHRV
            ))
        }

        return nights
    }
}

// MARK: - View

struct SleepChartView: View {
    @State private var nights: [SleepNight] = []
    @State private var isLoading = true
    @State private var loadError: String?

    private let loader = SleepChartLoader()

    // We need to scale HRV (typically 20–120 ms) onto the sleep minutes axis.
    // We compute the scale lazily from actual data.
    private var sleepAxisMax: Double {
        (nights.map { $0.deepMinutes + $0.coreMinutes + $0.remMinutes + $0.awakeMinutes }.max() ?? 60) * 1.15
    }

    private var hrvRange: (min: Double, max: Double) {
        let values = nights.compactMap(\.avgHRV)
        guard !values.isEmpty else { return (min: 0, max: 100) }
        let lo = values.min()!
        let hi = values.max()!
        // Add 10 % padding so the line never sits at the very bottom/top of
        // the chart area.
        return (min: lo * 0.9, max: hi * 1.1)
    }

    /// The actual (un-padded) HRV min and max, used for legend labels.
    private var hrvActualRange: (min: Double, max: Double) {
        let values = nights.compactMap(\.avgHRV)
        guard !values.isEmpty else { return (min: 0, max: 100) }
        return (min: values.min()!, max: values.max()!)
    }

    /// Convert an HRV value (ms) to a position on the sleep minutes axis.
    private func hrvToAxis(_ hrv: Double) -> Double {
        let range = hrvRange
        let fraction = (hrv - range.min) / (range.max - range.min)
        return fraction * sleepAxisMax
    }

    // Flat rows for stacked bars.
    private var bars: [SleepBar] {
        nights.flatMap { night in
            [
                SleepBar(night: night.id, stage: .deep,  minutes: night.deepMinutes),
                SleepBar(night: night.id, stage: .core,  minutes: night.coreMinutes),
                SleepBar(night: night.id, stage: .rem,   minutes: night.remMinutes),
                SleepBar(night: night.id, stage: .awake, minutes: night.awakeMinutes),
            ]
        }
    }

    // Date formatter for X axis labels (MM/dd).
    private static let axisDateFormatter: DateFormatter = {
        let f = DateFormatter()
        f.dateFormat = "MM/dd"
        return f
    }()

    var body: some View {
        GroupBox("Sleep & HRV — Last 14 Nights") {
            if isLoading {
                HStack {
                    Spacer()
                    ProgressView("Loading sleep data…")
                        .accessibilityIdentifier("sleepChartLoading")
                    Spacer()
                }
                .padding(.vertical, 40)
            } else if let error = loadError {
                Text(error)
                    .foregroundStyle(.secondary)
                    .font(.footnote)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.vertical, 24)
                    .accessibilityIdentifier("sleepChartError")
            } else if nights.isEmpty {
                Text("No sleep data for the past 14 nights")
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.vertical, 24)
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

    // MARK: Chart

    @ViewBuilder
    private var chartContent: some View {
        VStack(alignment: .leading, spacing: 8) {
            Chart {
                // Stacked sleep stage bars
                ForEach(bars) { bar in
                    BarMark(
                        x: .value("Night", bar.night),
                        y: .value("Minutes", bar.minutes),
                        stacking: .standard
                    )
                    .foregroundStyle(by: .value("Stage", bar.stage.rawValue))
                    .cornerRadius(2)
                }

                // HRV line overlay (scaled to sleep minutes axis)
                ForEach(nights.filter { $0.avgHRV != nil }) { night in
                    LineMark(
                        x: .value("Night", night.id),
                        y: .value("HRV (scaled)", hrvToAxis(night.avgHRV!))
                    )
                    .foregroundStyle(.red.opacity(0.85))
                    .interpolationMethod(.catmullRom)
                    .lineStyle(StrokeStyle(lineWidth: 2, dash: [4, 2]))

                    PointMark(
                        x: .value("Night", night.id),
                        y: .value("HRV (scaled)", hrvToAxis(night.avgHRV!))
                    )
                    .foregroundStyle(.red.opacity(0.85))
                    .symbolSize(30)
                }
            }
            .chartForegroundStyleScale([
                SleepStage.deep.rawValue:  SleepStage.deep.color,
                SleepStage.core.rawValue:  SleepStage.core.color,
                SleepStage.rem.rawValue:   SleepStage.rem.color,
                SleepStage.awake.rawValue: SleepStage.awake.color,
            ])
            .chartXAxis {
                AxisMarks(values: nights.map(\.id)) { value in
                    AxisGridLine()
                    AxisTick()
                    AxisValueLabel {
                        if let date = value.as(Date.self) {
                            Text(Self.axisDateFormatter.string(from: date))
                                .font(.caption2)
                        }
                    }
                }
            }
            .chartYAxis {
                // Primary axis: sleep minutes (left side)
                AxisMarks(position: .leading) { value in
                    AxisGridLine()
                    AxisTick()
                    AxisValueLabel {
                        if let minutes = value.as(Double.self) {
                            // Only show the raw minutes label if it falls in the
                            // sleep range (below sleepAxisMax).
                            Text("\(Int(minutes)) min")
                                .font(.caption2)
                        }
                    }
                }
            }
            .chartYScale(domain: 0...sleepAxisMax)
            .frame(height: 220)

            // Secondary-axis legend row (HRV values at top/bottom of its range,
            // since Swift Charts does not natively support dual Y axes).
            if nights.contains(where: { $0.avgHRV != nil }) {
                hrvScaleLegend
            }

            // Stage legend
            stageLegend
        }
    }

    // Explains the HRV scale that was projected onto the minutes axis.
    private var hrvScaleLegend: some View {
        HStack(spacing: 6) {
            // Dashed line swatch drawn with a stroked Path.
            Canvas { context, size in
                var path = Path()
                path.move(to: CGPoint(x: 0, y: size.height / 2))
                path.addLine(to: CGPoint(x: size.width, y: size.height / 2))
                context.stroke(
                    path,
                    with: .color(.red.opacity(0.85)),
                    style: StrokeStyle(lineWidth: 1.5, dash: [4, 2])
                )
            }
            .frame(width: 22, height: 8)

            Text("HRV overlay")
                .font(.caption2)
                .foregroundStyle(.secondary)

            Spacer()

            let range = hrvActualRange
            Text(String(format: "%.0f–%.0f ms", range.min, range.max))
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
        .accessibilityIdentifier("sleepChartHRVLegend")
    }

    private var stageLegend: some View {
        HStack(spacing: 10) {
            ForEach(SleepStage.allCases, id: \.self) { stage in
                HStack(spacing: 4) {
                    RoundedRectangle(cornerRadius: 3)
                        .fill(stage.color)
                        .frame(width: 12, height: 12)
                    Text(stage.rawValue)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }
            Spacer()
        }
        .accessibilityIdentifier("sleepChartStageLegend")
    }

    // MARK: Data loading

    private func loadData() async {
        isLoading = true
        loadError = nil
        do {
            nights = try await loader.load()
        } catch {
            loadError = "Could not load sleep data: \(error.localizedDescription)"
        }
        isLoading = false
    }
}
