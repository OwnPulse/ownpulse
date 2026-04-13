// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct HealthOverviewView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var viewModel: ExploreViewModel?
    @State private var hiddenSubstances: Set<String> = []

    private static let metricColors: [String: Color] = [
        "body_mass": OPColor.terracotta,
        "heart_rate": OPColor.teal,
        "sleep_analysis": OPColor.sage,
    ]

    var body: some View {
        ScrollView {
            if let vm = viewModel {
                overviewContent(vm: vm)
            } else {
                ProgressView()
                    .frame(maxWidth: .infinity, minHeight: 200)
            }
        }
        .navigationTitle("Health Overview")
        .navigationBarTitleDisplayMode(.inline)
        .task {
            if viewModel == nil {
                let vm = ExploreViewModel(networkClient: dependencies.networkClient)
                viewModel = vm
                vm.loadHealthOverviewPreset()
            }
        }
    }

    @ViewBuilder
    private func overviewContent(vm: ExploreViewModel) -> some View {
        VStack(spacing: 16) {
            // Date range picker
            DateRangeSegment(selection: Binding(
                get: { vm.dateRange },
                set: { vm.setDateRange($0) }
            ))
            .padding(.horizontal, 16)

            // Chart
            switch vm.loadState {
            case .idle, .loading:
                ProgressView()
                    .frame(maxWidth: .infinity, minHeight: 200)
                    .accessibilityIdentifier("healthOverviewLoading")

            case .error(let message):
                VStack(spacing: 8) {
                    Image(systemName: "exclamationmark.triangle")
                        .font(.title2)
                        .foregroundStyle(OPColor.terracotta)
                    Text(message)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Button("Retry") {
                        vm.loadHealthOverviewPreset()
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(OPColor.terracotta)
                }
                .frame(maxWidth: .infinity, minHeight: 200)
                .accessibilityIdentifier("healthOverviewError")

            case .loaded:
                chartSection(vm: vm)
            }

            // Legend
            legendSection(vm: vm)

            // Substance filter pills
            if !vm.interventions.isEmpty {
                substanceFilterSection(vm: vm)
            }
        }
        .padding(.vertical, 12)
    }

    @ViewBuilder
    private func chartSection(vm: ExploreViewModel) -> some View {
        let chartMetrics = vm.seriesData.map { series in
            let color = Self.metricColors[series.field] ?? OPColor.teal
            let alwaysMA = series.field == "body_mass"
            let showMA = vm.showMovingAverage || alwaysMA
            let points = series.points.compactMap { point -> ChartPoint? in
                guard let date = ISO8601DateFormatter().date(from: point.t) else { return nil }
                return ChartPoint(date: date, value: point.v)
            }
            let maPoints: [ChartPoint]? = showMA
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
            hiddenSubstances: hiddenSubstances,
            height: UIScreen.main.bounds.height * 0.4,
            showMovingAverage: true
        )
        .padding(.horizontal, 16)
        .accessibilityIdentifier("healthOverviewChart")
    }

    @ViewBuilder
    private func legendSection(vm: ExploreViewModel) -> some View {
        HStack(spacing: 16) {
            ForEach(vm.seriesData, id: \.field) { series in
                let color = Self.metricColors[series.field] ?? OPColor.teal
                HStack(spacing: 6) {
                    Circle()
                        .fill(color)
                        .frame(width: 10, height: 10)
                    Text(series.field.replacingOccurrences(of: "_", with: " ").capitalized)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .padding(.horizontal, 16)
        .accessibilityIdentifier("healthOverviewLegend")
    }

    @ViewBuilder
    private func substanceFilterSection(vm: ExploreViewModel) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Interventions")
                .font(.headline)
                .padding(.leading, 4)

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(uniqueSubstances(vm: vm), id: \.self) { substance in
                        let isHidden = hiddenSubstances.contains(substance)
                        Button {
                            if isHidden {
                                hiddenSubstances.remove(substance)
                            } else {
                                hiddenSubstances.insert(substance)
                            }
                        } label: {
                            Text(substance)
                                .font(.caption)
                                .fontWeight(.medium)
                                .padding(.horizontal, 12)
                                .padding(.vertical, 6)
                                .background(
                                    Capsule()
                                        .fill(isHidden ? Color.clear : OPColor.gold.opacity(0.2))
                                )
                                .overlay(
                                    Capsule()
                                        .strokeBorder(OPColor.gold.opacity(0.4), lineWidth: 1)
                                )
                                .foregroundStyle(isHidden ? .secondary : OPColor.gold)
                        }
                        .accessibilityIdentifier("substanceFilter-\(substance)")
                    }
                }
            }
        }
        .padding(.horizontal, 16)
        .accessibilityIdentifier("substanceFilterSection")
    }

    private func uniqueSubstances(vm: ExploreViewModel) -> [String] {
        Array(Set(vm.interventions.map(\.substance))).sorted()
    }
}
