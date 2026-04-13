// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Charts
import SwiftUI

struct MetricDetailView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var viewModel: ExploreViewModel?
    @State private var showMetricPicker = false

    let source: String
    let field: String
    let metricLabel: String
    let metricUnit: String

    var body: some View {
        ScrollView {
            if let vm = viewModel {
                detailContent(vm: vm)
            } else {
                ProgressView()
                    .frame(maxWidth: .infinity, minHeight: 200)
            }
        }
        .navigationTitle(metricLabel)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    showMetricPicker = true
                } label: {
                    Label("Compare", systemImage: "plus.circle")
                }
                .accessibilityIdentifier("compareButton")
            }
        }
        .sheet(isPresented: $showMetricPicker) {
            if let vm = viewModel {
                metricPickerSheet(vm: vm)
            }
        }
        .task {
            if viewModel == nil {
                let vm = ExploreViewModel(networkClient: dependencies.networkClient)
                vm.selectMetric(MetricSpec(source: source, field: field))
                viewModel = vm
            }
            await viewModel?.loadMetrics()
            await viewModel?.loadInterventions()
        }
    }

    @ViewBuilder
    private func detailContent(vm: ExploreViewModel) -> some View {
        VStack(spacing: 16) {
            // Date range picker
            DateRangeSegment(selection: Binding(
                get: { vm.dateRange },
                set: { vm.setDateRange($0) }
            ))
            .padding(.horizontal, 16)

            // Chart
            switch vm.loadState {
            case .idle:
                EmptyView()

            case .loading:
                ProgressView()
                    .frame(maxWidth: .infinity, minHeight: 200)
                    .accessibilityIdentifier("metricDetailLoading")

            case .error(let message):
                VStack(spacing: 8) {
                    Image(systemName: "exclamationmark.triangle")
                        .font(.title2)
                        .foregroundStyle(OPColor.terracotta)
                    Text(message)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, minHeight: 200)
                .accessibilityIdentifier("metricDetailError")

            case .loaded:
                chartSection(vm: vm)
            }

            // Moving average toggle
            Toggle("7-Day Average", isOn: Binding(
                get: { vm.showMovingAverage },
                set: { vm.showMovingAverage = $0 }
            ))
            .font(.subheadline)
            .padding(.horizontal, 16)
            .accessibilityIdentifier("movingAverageToggle")

            // Summary stats
            if let series = vm.seriesData.first(where: { $0.field == field }) {
                summaryStatsCard(series: series)
            }

            // Interventions list
            if !vm.interventions.isEmpty {
                interventionsList(vm: vm)
            }
        }
        .padding(.vertical, 12)
    }

    @ViewBuilder
    private func chartSection(vm: ExploreViewModel) -> some View {
        let chartMetrics = vm.seriesData.map { series in
            let color = colorForMetric(series.field)
            let points = series.points.compactMap { point -> ChartPoint? in
                guard let date = ISO8601DateFormatter().date(from: point.t) else { return nil }
                return ChartPoint(date: date, value: point.v)
            }
            let maPoints: [ChartPoint]? = vm.showMovingAverage
                ? movingAverage(points: series.points, window: 7).compactMap { point in
                    guard let date = ISO8601DateFormatter().date(from: point.t) else { return nil }
                    return ChartPoint(date: date, value: point.v)
                }
                : nil
            return ChartMetric(
                field: series.field,
                label: series.field.replacingOccurrences(of: "_", with: " ").capitalized,
                unit: series.unit,
                color: color,
                points: points,
                maPoints: maPoints
            )
        }

        OverlayChartView(
            metrics: chartMetrics,
            interventions: vm.interventions,
            hiddenSubstances: [],
            height: UIScreen.main.bounds.height * 0.4,
            showMovingAverage: vm.showMovingAverage
        )
        .padding(.horizontal, 16)
        .accessibilityIdentifier("metricDetailChart")
    }

    @ViewBuilder
    private func summaryStatsCard(series: SeriesData) -> some View {
        let values = series.points.map(\.v)
        let avg = values.isEmpty ? 0 : values.reduce(0, +) / Double(values.count)
        let min = values.min() ?? 0
        let max = values.max() ?? 0

        HStack(spacing: 0) {
            statItem(title: "Avg", value: String(format: "%.1f", avg), unit: series.unit)
            Divider().frame(height: 40)
            statItem(title: "Min", value: String(format: "%.1f", min), unit: series.unit)
            Divider().frame(height: 40)
            statItem(title: "Max", value: String(format: "%.1f", max), unit: series.unit)
        }
        .opCard()
        .padding(.horizontal, 16)
        .accessibilityIdentifier("summaryStats")
    }

    private func statItem(title: String, value: String, unit: String) -> some View {
        VStack(spacing: 4) {
            Text(title)
                .font(.caption2)
                .foregroundStyle(.secondary)
            Text(value)
                .font(.system(.title3, design: .rounded, weight: .bold))
            Text(unit)
                .font(.caption2)
                .foregroundStyle(.tertiary)
        }
        .frame(maxWidth: .infinity)
    }

    @ViewBuilder
    private func interventionsList(vm: ExploreViewModel) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Interventions")
                .font(.headline)
                .padding(.leading, 4)

            ForEach(vm.interventions.prefix(10)) { marker in
                HStack {
                    Circle()
                        .fill(OPColor.gold)
                        .frame(width: 8, height: 8)
                    Text(marker.substance)
                        .font(.subheadline)
                    Spacer()
                    if let dose = marker.dose, let unit = marker.unit {
                        Text("\(String(format: "%.0f", dose)) \(unit)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Text(marker.date, format: .dateTime.month(.abbreviated).day())
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                }
                .padding(.vertical, 4)
            }
        }
        .padding(.horizontal, 16)
        .accessibilityIdentifier("interventionsList")
    }

    @ViewBuilder
    private func metricPickerSheet(vm: ExploreViewModel) -> some View {
        NavigationStack {
            List {
                ForEach(vm.metrics) { group in
                    Section(group.label) {
                        ForEach(group.metrics) { item in
                            Button {
                                vm.selectMetric(MetricSpec(source: group.source, field: item.field))
                                showMetricPicker = false
                            } label: {
                                HStack {
                                    Text(item.label)
                                    Spacer()
                                    if vm.selectedMetrics.contains(where: { $0.field == item.field }) {
                                        Image(systemName: "checkmark")
                                            .foregroundStyle(OPColor.terracotta)
                                    }
                                }
                            }
                            .accessibilityIdentifier("metricPickerItem-\(item.field)")
                        }
                    }
                }
            }
            .navigationTitle("Add Metric")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { showMetricPicker = false }
                        .accessibilityIdentifier("metricPickerCancel")
                }
            }
        }
        .presentationDetents([.medium, .large])
    }

    // MARK: - Color assignment

    private static let metricColors: [Color] = [
        OPColor.terracotta, OPColor.teal, OPColor.sage, OPColor.gold,
        Color.purple,
    ]

    private func colorForMetric(_ field: String) -> Color {
        guard let vm = viewModel else { return OPColor.terracotta }
        let index = vm.selectedMetrics.firstIndex(where: { $0.field == field }) ?? 0
        return Self.metricColors[index % Self.metricColors.count]
    }
}
