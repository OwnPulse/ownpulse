// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import Charts
import SwiftUI

struct ExploreView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var viewModel: ExploreViewModel?

    var body: some View {
        ScrollView {
            if let vm = viewModel {
                exploreContent(vm: vm)
            } else {
                ProgressView()
                    .frame(maxWidth: .infinity, minHeight: 200)
            }
        }
        .navigationTitle("Explore")
        .navigationBarTitleDisplayMode(.large)
        .task {
            if viewModel == nil {
                viewModel = ExploreViewModel(networkClient: dependencies.networkClient)
            }
            await viewModel?.loadMetrics()
        }
    }

    @ViewBuilder
    private func exploreContent(vm: ExploreViewModel) -> some View {
        LazyVStack(spacing: 16) {
            switch vm.metricsLoadState {
            case .idle, .loading:
                ProgressView()
                    .frame(maxWidth: .infinity, minHeight: 200)
                    .accessibilityIdentifier("exploreLoading")

            case .error(let message):
                VStack(spacing: 12) {
                    Image(systemName: "exclamationmark.triangle")
                        .font(.largeTitle)
                        .foregroundStyle(OPColor.terracotta)
                    Text(message)
                        .foregroundStyle(.secondary)
                    Button("Retry") {
                        Task { await vm.loadMetrics() }
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(OPColor.terracotta)
                }
                .frame(maxWidth: .infinity, minHeight: 200)
                .accessibilityIdentifier("exploreError")

            case .loaded:
                // Featured Health Overview card
                NavigationLink {
                    HealthOverviewView()
                } label: {
                    healthOverviewCard
                }
                .accessibilityIdentifier("healthOverviewCard")

                // Metric category sections
                ForEach(vm.metrics) { group in
                    metricCategorySection(group, vm: vm)
                }
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    // MARK: - Health Overview featured card

    private var healthOverviewCard: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 6) {
                Label("Health Overview", systemImage: "heart.text.clipboard")
                    .font(.headline)
                    .foregroundStyle(OPColor.terracotta)

                Text("Weight, heart rate & sleep overlaid with your interventions")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }

            Spacer()

            Image(systemName: "chevron.right")
                .font(.caption)
                .foregroundStyle(.tertiary)
        }
        .opCard()
    }

    // MARK: - Metric category section

    @ViewBuilder
    private func metricCategorySection(_ group: MetricSourceGroup, vm: ExploreViewModel) -> some View {
        ExploreMetricCategorySection(group: group, vm: vm)
    }
}

/// One category section on the Explore tab. Isolated to its own `View` so
/// SwiftUI can scope the `.task(id:)` modifier to this specific section —
/// the task is automatically cancelled when the section leaves the
/// hierarchy, preventing detached fetches from mutating `@Observable` state
/// after dismount.
private struct ExploreMetricCategorySection: View {
    let group: MetricSourceGroup
    let vm: ExploreViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(group.label)
                .font(.headline)
                .foregroundStyle(.primary)
                .padding(.leading, 4)

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 12) {
                    ForEach(group.metrics) { item in
                        NavigationLink {
                            MetricDetailView(
                                source: group.source,
                                field: item.field,
                                metricLabel: item.label,
                                metricUnit: item.unit
                            )
                        } label: {
                            MetricBrowseCard(
                                source: group.source,
                                field: item.field,
                                label: item.label,
                                unit: item.unit,
                                points: vm.sparklineData[ExploreViewModel.sparklineKey(source: group.source, field: item.field)],
                                isLoading: vm.sparklineLoadingSections.contains(ExploreViewModel.sparklineKey(source: group.source, field: item.field))
                            )
                        }
                        .accessibilityIdentifier("metricCard-\(item.field)")
                    }
                }
            }
        }
        .accessibilityIdentifier("metricCategory-\(group.source)")
        // `.task(id:)` handles cancellation for free — if the section leaves
        // the hierarchy or `group.source` changes, the in-flight fetch is
        // cancelled, so the VM won't try to mutate state after dismount.
        .task(id: group.source) {
            await vm.loadSparklines(source: group.source, fields: group.metrics.map(\.field))
        }
    }
}

// MARK: - Metric Browse Card

private struct MetricBrowseCard: View {
    let source: String
    let field: String
    let label: String
    let unit: String
    let points: [DataPoint]?
    let isLoading: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)

            Text(BrowseCardPresenter.displayUnit(field: field, unit: unit))
                .font(.system(.caption2, design: .rounded))
                .foregroundStyle(.tertiary)

            sparkline
                .frame(height: 36)

            if let value = BrowseCardPresenter.latestValueText(field: field, points: points) {
                Text(value)
                    .font(.system(.footnote, design: .rounded, weight: .semibold))
                    .foregroundStyle(.primary)
                    .accessibilityIdentifier("metricCardValue-\(field)")
            } else {
                Text("—")
                    .font(.system(.footnote, design: .rounded, weight: .semibold))
                    .foregroundStyle(.tertiary)
                    .accessibilityIdentifier("metricCardValue-\(field)")
            }
        }
        .frame(width: 130)
        .opCard()
    }

    @ViewBuilder
    private var sparkline: some View {
        switch BrowseCardPresenter.sparklineState(points: points, isLoading: isLoading) {
        case .chart(let pts):
            MetricSparklineChart(points: pts)
                .accessibilityIdentifier("metricCardSparkline-\(field)")
        case .loading:
            HStack {
                Spacer()
                ProgressView()
                    .controlSize(.mini)
                Spacer()
            }
            .accessibilityIdentifier("metricCardLoading-\(field)")
        case .empty:
            RoundedRectangle(cornerRadius: 4)
                .fill(OPColor.teal.opacity(0.1))
                .overlay {
                    Image(systemName: "chart.xyaxis.line")
                        .font(.caption2)
                        .foregroundStyle(OPColor.teal.opacity(0.3))
                }
                .accessibilityIdentifier("metricCardPlaceholder-\(field)")
        }
    }
}

// MARK: - Sparkline

private struct SparklinePoint: Identifiable {
    let index: Int
    let value: Double
    var id: Int { index }
}

private struct MetricSparklineChart: View {
    let points: [DataPoint]

    private var chartPoints: [SparklinePoint] {
        points.enumerated().map { i, p in SparklinePoint(index: i, value: p.v) }
    }

    var body: some View {
        Chart(chartPoints) { point in
            LineMark(
                x: .value("Index", point.index),
                y: .value("Value", point.value)
            )
            .foregroundStyle(OPColor.teal)
            .interpolationMethod(.monotone)
            .lineStyle(StrokeStyle(lineWidth: 1.5, lineCap: .round))
        }
        .chartYScale(domain: .automatic(includesZero: ChartAxisConfig.includesZeroInYAxis))
        .chartXAxis(.hidden)
        .chartYAxis(.hidden)
    }
}
