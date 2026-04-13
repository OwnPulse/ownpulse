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
                                label: item.label,
                                unit: item.unit
                            )
                        }
                        .accessibilityIdentifier("metricCard-\(item.field)")
                    }
                }
            }
        }
        .accessibilityIdentifier("metricCategory-\(group.source)")
    }
}

// MARK: - Metric Browse Card

private struct MetricBrowseCard: View {
    let label: String
    let unit: String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)

            Text(unit)
                .font(.system(.caption2, design: .rounded))
                .foregroundStyle(.tertiary)

            // Placeholder sparkline area
            RoundedRectangle(cornerRadius: 4)
                .fill(OPColor.teal.opacity(0.1))
                .frame(height: 40)
                .overlay {
                    Image(systemName: "chart.xyaxis.line")
                        .font(.caption2)
                        .foregroundStyle(OPColor.teal.opacity(0.3))
                }
        }
        .frame(width: 130)
        .opCard()
    }
}
