// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct DashboardView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var viewModel: DashboardViewModel?

    var body: some View {
        ScrollView {
            if let vm = viewModel {
                dashboardContent(vm: vm)
            } else {
                ProgressView()
                    .frame(maxWidth: .infinity, minHeight: 200)
            }
        }
        .refreshable {
            await viewModel?.loadDashboard()
        }
        .navigationTitle("Dashboard")
        .navigationBarTitleDisplayMode(.large)
        .background(backgroundGradient)
        .onAppear {
            if viewModel == nil {
                viewModel = DashboardViewModel(
                    networkClient: dependencies.networkClient,
                    syncEngine: dependencies.syncEngine
                )
            }
            Task { await viewModel?.loadDashboard() }
        }
    }

    @ViewBuilder
    private func dashboardContent(vm: DashboardViewModel) -> some View {
        LazyVStack(spacing: 16) {
            switch vm.summaryState {
            case .idle, .loading:
                ProgressView()
                    .frame(maxWidth: .infinity, minHeight: 200)
                    .accessibilityIdentifier("dashboardLoading")

            case .error(let message):
                VStack(spacing: 12) {
                    Image(systemName: "exclamationmark.triangle")
                        .font(.largeTitle)
                        .foregroundStyle(OPColor.terracotta)
                    Text(message)
                        .foregroundStyle(.secondary)
                    Button("Retry") {
                        Task { await vm.loadDashboard() }
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(OPColor.terracotta)
                }
                .frame(maxWidth: .infinity, minHeight: 200)
                .accessibilityIdentifier("dashboardError")

            case .loaded:
                // Hero Metric Card
                if !vm.heroSeries.isEmpty {
                    HeroMetricCard(
                        metricName: vm.heroMetricName,
                        currentValue: vm.heroCurrentValue,
                        unit: vm.heroMetricUnit,
                        trendText: vm.heroTrendText,
                        trendIsPositive: vm.heroTrendIsPositive,
                        dataPoints: vm.heroSeries
                    )
                    .transition(.move(edge: .bottom).combined(with: .opacity))
                    .accessibilityIdentifier("heroMetricCard")
                }

                // Today's Check-in Card
                CheckinSummaryCard(
                    latestCheckin: vm.summary?.latestCheckin
                )
                .transition(.move(edge: .bottom).combined(with: .opacity))
                .accessibilityIdentifier("checkinSummaryCard")

                // 7-Day Sparklines
                if !vm.sparklines.isEmpty {
                    sparklineSection(vm: vm)
                }

                // Insight Cards
                ForEach(vm.insights) { insight in
                    InsightCardView(insight: insight) {
                        withAnimation(.spring(duration: 0.3)) {
                            vm.dismissInsight(insight)
                        }
                    }
                    .transition(.slide)
                    .accessibilityIdentifier("insightCard-\(insight.id)")
                }

                // Weekly Summary
                if let summary = vm.summary {
                    WeeklySummaryCard(summary: summary)
                        .transition(.move(edge: .bottom).combined(with: .opacity))
                        .accessibilityIdentifier("weeklySummaryCard")
                }

                // Sync Status
                syncStatusRow(vm: vm)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .animation(.spring(duration: 0.5), value: vm.summaryState == .loaded)
    }

    @ViewBuilder
    private func sparklineSection(vm: DashboardViewModel) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("7-Day Trends")
                .font(.headline)
                .foregroundStyle(.primary)
                .padding(.leading, 4)

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 12) {
                    ForEach(vm.sparklines) { series in
                        SparklineCard(series: series)
                            .accessibilityIdentifier("sparkline-\(series.field)")
                    }
                }
            }
        }
        .accessibilityIdentifier("sparklineSection")
    }

    @ViewBuilder
    private func syncStatusRow(vm: DashboardViewModel) -> some View {
        HStack {
            if let lastSync = vm.lastSyncDate {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundStyle(OPColor.sage)
                    .font(.caption)
                Text("Last synced \(lastSync, format: .relative(presentation: .named))")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                Image(systemName: "arrow.triangle.2.circlepath")
                    .foregroundStyle(.secondary)
                    .font(.caption)
                Text("Pull to sync")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)
        .accessibilityIdentifier("syncStatus")
    }

    @Environment(\.colorScheme) private var colorScheme

    private var backgroundGradient: some View {
        Group {
            if colorScheme == .dark {
                LinearGradient(
                    colors: [OPColor.darkBg, Color(red: 0.08, green: 0.08, blue: 0.1)],
                    startPoint: .top,
                    endPoint: .bottom
                )
                .ignoresSafeArea()
            } else {
                LinearGradient(
                    colors: [OPColor.warmBg, Color.white],
                    startPoint: .top,
                    endPoint: .bottom
                )
                .ignoresSafeArea()
            }
        }
    }
}

// Equatable conformance for state comparison in animation
extension DashboardViewModel.LoadState: Equatable {
    static func == (lhs: DashboardViewModel.LoadState, rhs: DashboardViewModel.LoadState) -> Bool {
        switch (lhs, rhs) {
        case (.idle, .idle), (.loading, .loading), (.loaded, .loaded):
            return true
        case (.error(let a), .error(let b)):
            return a == b
        default:
            return false
        }
    }
}
