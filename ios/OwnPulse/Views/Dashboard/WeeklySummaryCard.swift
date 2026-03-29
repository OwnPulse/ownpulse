// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct WeeklySummaryCard: View {
    let summary: DashboardSummary

    private struct StatItem {
        let label: String
        let value: Int
        let icon: String
        let color: Color
    }

    private var stats: [StatItem] {
        [
            StatItem(label: "Check-ins", value: summary.checkinCount7d, icon: "checklist", color: OPColor.terracotta),
            StatItem(label: "Health Records", value: summary.healthRecordCount7d, icon: "heart.fill", color: OPColor.teal),
            StatItem(label: "Interventions", value: summary.interventionCount7d, icon: "pills.fill", color: OPColor.gold),
            StatItem(label: "Observations", value: summary.observationCount7d, icon: "eye.fill", color: OPColor.sage),
        ]
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("This Week")
                .font(.headline)
                .foregroundStyle(.primary)

            LazyVGrid(columns: [
                GridItem(.flexible()),
                GridItem(.flexible()),
            ], spacing: 12) {
                ForEach(stats, id: \.label) { stat in
                    statCell(stat)
                }
            }
        }
        .opCard()
    }

    @ViewBuilder
    private func statCell(_ stat: StatItem) -> some View {
        HStack(spacing: 10) {
            Image(systemName: stat.icon)
                .font(.title3)
                .foregroundStyle(stat.color)
                .frame(width: 28)

            VStack(alignment: .leading, spacing: 2) {
                Text("\(stat.value)")
                    .font(.system(.title3, design: .rounded, weight: .bold))

                Text(stat.label)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityIdentifier("weeklyStat-\(stat.label.lowercased().replacingOccurrences(of: " ", with: "-"))")
    }
}
