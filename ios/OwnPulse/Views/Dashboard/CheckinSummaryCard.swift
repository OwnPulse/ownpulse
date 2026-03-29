// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct CheckinSummaryCard: View {
    let latestCheckin: LatestCheckin?

    @State private var pulseAnimation = false

    private struct ScoreItem {
        let label: String
        let value: Int?
        let color: Color
    }

    private var scores: [ScoreItem] {
        guard let checkin = latestCheckin else { return [] }
        return [
            ScoreItem(label: "Energy", value: checkin.energy, color: OPColor.gold),
            ScoreItem(label: "Mood", value: checkin.mood, color: OPColor.terracotta),
            ScoreItem(label: "Focus", value: checkin.focus, color: OPColor.teal),
            ScoreItem(label: "Recovery", value: checkin.recovery, color: OPColor.sage),
            ScoreItem(label: "Libido", value: checkin.libido, color: Color.purple),
        ]
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Today's Check-in")
                .font(.headline)
                .foregroundStyle(.primary)

            if let checkin = latestCheckin, checkin.isToday {
                // Show score rings
                HStack(spacing: 16) {
                    ForEach(scores, id: \.label) { score in
                        scoreRing(item: score)
                    }
                }
                .frame(maxWidth: .infinity)

                Text("Logged \(formattedTime(from: checkin.date))")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                // Prompt to log
                Button {
                    // Navigation handled by parent via tab switch
                } label: {
                    HStack {
                        Image(systemName: "plus.circle.fill")
                            .font(.title2)
                            .foregroundStyle(OPColor.terracotta)
                            .scaleEffect(pulseAnimation ? 1.1 : 1.0)
                        Text("Log Today's Check-in")
                            .fontWeight(.medium)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 12)
                    .background(
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .fill(OPColor.terracotta.opacity(0.1))
                    )
                }
                .accessibilityIdentifier("logCheckinPrompt")
                .onAppear {
                    withAnimation(.easeInOut(duration: 1.2).repeatForever(autoreverses: true)) {
                        pulseAnimation = true
                    }
                }
            }
        }
        .opCard()
    }

    @ViewBuilder
    private func scoreRing(item: ScoreItem) -> some View {
        let value = item.value ?? 0
        let progress = Double(value) / 10.0

        VStack(spacing: 4) {
            ZStack {
                Circle()
                    .stroke(item.color.opacity(0.2), lineWidth: 4)
                    .frame(width: 44, height: 44)

                Circle()
                    .trim(from: 0, to: progress)
                    .stroke(item.color, style: StrokeStyle(lineWidth: 4, lineCap: .round))
                    .frame(width: 44, height: 44)
                    .rotationEffect(.degrees(-90))

                Text("\(value)")
                    .font(.system(.callout, design: .rounded, weight: .bold))
            }
            .accessibilityIdentifier("scoreRing-\(item.label.lowercased())")

            Text(item.label)
                .font(.system(size: 10))
                .foregroundStyle(.secondary)
        }
    }

    private func formattedTime(from dateString: String) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = formatter.date(from: dateString) {
            let timeFormatter = DateFormatter()
            timeFormatter.timeStyle = .short
            return timeFormatter.string(from: date)
        }
        // Try without fractional seconds
        formatter.formatOptions = [.withInternetDateTime]
        if let date = formatter.date(from: dateString) {
            let timeFormatter = DateFormatter()
            timeFormatter.timeStyle = .short
            return timeFormatter.string(from: date)
        }
        return dateString
    }
}
