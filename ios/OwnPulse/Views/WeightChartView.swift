// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Charts
import SwiftUI

struct WeightPoint: Identifiable {
    let id: String
    let date: Date
    let value: Double
}

struct WeightChartView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var points: [WeightPoint] = []
    @State private var isLoading = true
    @State private var error: String?

    var body: some View {
        GroupBox("Weight") {
            if isLoading {
                ProgressView()
                    .frame(maxWidth: .infinity, minHeight: 200)
                    .accessibilityIdentifier("weightChartLoading")
            } else if let error {
                Text(error)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, minHeight: 60)
                    .accessibilityIdentifier("weightChartError")
            } else if points.isEmpty {
                Text("No weight data for the last 90 days.")
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, minHeight: 60)
                    .accessibilityIdentifier("weightChartEmpty")
            } else {
                chartContent
                    .accessibilityIdentifier("weightChart")
            }
        }
        .task {
            await loadData()
        }
    }

    @ViewBuilder
    private var chartContent: some View {
        Chart(points) { point in
            LineMark(
                x: .value("Date", point.date, unit: .day),
                y: .value("kg", point.value)
            )
            .foregroundStyle(Color(hex: 0x3D8B8B))
            .interpolationMethod(.catmullRom)

            PointMark(
                x: .value("Date", point.date, unit: .day),
                y: .value("kg", point.value)
            )
            .foregroundStyle(Color(hex: 0x3D8B8B))
            .symbolSize(20)
        }
        .chartYAxis {
            AxisMarks(position: .leading) { value in
                AxisGridLine()
                AxisValueLabel {
                    if let kg = value.as(Double.self) {
                        Text(String(format: "%.1f", kg))
                    }
                }
            }
        }
        .chartXAxis {
            AxisMarks(values: .stride(by: .day, count: 14)) { _ in
                AxisValueLabel(format: .dateTime.month(.abbreviated).day())
            }
        }
        .frame(height: 260)

        // Summary
        if let latest = points.last, let earliest = points.first {
            let delta = latest.value - earliest.value
            HStack {
                Text("Latest: \(String(format: "%.1f", latest.value)) kg")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                Text("\(delta >= 0 ? "+" : "")\(String(format: "%.1f", delta)) kg over period")
                    .font(.caption)
                    .foregroundStyle(delta >= 0 ? .orange : .green)
            }
            .accessibilityIdentifier("weightChartSummary")
        }
    }

    private func loadData() async {
        let calendar = Calendar.current
        let since = calendar.date(byAdding: .day, value: -90, to: Date())!
        let sinceStr = ISO8601DateFormatter().string(from: since)
        let path = "\(Endpoints.healthRecords)?record_type=body_mass&start=\(sinceStr)"

        do {
            let records: [HealthRecordResponse] = try await dependencies.networkClient.request(
                method: "GET",
                path: path,
                body: nil as String?
            )
            points = records
                .compactMap { record -> WeightPoint? in
                    WeightPoint(id: record.id, date: record.startTime, value: record.value)
                }
                .sorted { $0.date < $1.date }
        } catch {
            self.error = "Failed to load weight data"
        }

        isLoading = false
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
