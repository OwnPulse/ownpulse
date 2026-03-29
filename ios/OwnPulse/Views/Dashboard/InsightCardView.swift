// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct InsightCardView: View {
    let insight: Insight
    let onDismiss: () -> Void

    private var accentColor: Color {
        switch insight.insightType {
        case "correlation": return OPColor.teal
        case "trend": return OPColor.terracotta
        case "anomaly": return OPColor.gold
        default: return OPColor.sage
        }
    }

    var body: some View {
        HStack(spacing: 0) {
            // Left accent bar
            Rectangle()
                .fill(accentColor)
                .frame(width: 4)

            VStack(alignment: .leading, spacing: 6) {
                Text(insight.headline)
                    .font(.subheadline)
                    .fontWeight(.semibold)
                    .lineLimit(2)

                if let detail = insight.detail {
                    Text(detail)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(3)
                }

                Button {
                    if let url = URL(string: "\(AppConfig.webDashboardURL)/explore") {
                        UIApplication.shared.open(url)
                    }
                } label: {
                    Text("View")
                        .font(.caption)
                        .fontWeight(.medium)
                        .foregroundStyle(accentColor)
                }
                .accessibilityIdentifier("insightViewButton-\(insight.id)")
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)

            Button {
                onDismiss()
            } label: {
                Image(systemName: "xmark")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .padding(12)
            }
            .accessibilityIdentifier("insightDismiss-\(insight.id)")
        }
        .background(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(.ultraThinMaterial)
        )
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .shadow(color: .black.opacity(0.06), radius: 6, y: 3)
    }
}
