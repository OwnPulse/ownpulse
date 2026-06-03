// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import HealthKit
import SwiftUI

struct HealthKitPromptSheet: View {
    let onConnect: () -> Void
    let onDismiss: () -> Void

    @ScaledMetric(relativeTo: .largeTitle) private var iconSize: CGFloat = 64

    var body: some View {
        VStack(spacing: 24) {
            Spacer()

            Image(systemName: "heart.text.square.fill")
                .font(.system(size: iconSize))
                .foregroundStyle(OPColor.terracotta)
                .accessibilityIdentifier("healthKitPromptIcon")
                .accessibilityHidden(true)

            Text("Connect Apple Health")
                .font(.title2)
                .fontWeight(.semibold)
                .accessibilityIdentifier("healthKitPromptTitle")

            Text("Sync heart rate, sleep, activity, nutrition, and more from Apple Health into OwnPulse.")
                .font(.body)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
                .accessibilityIdentifier("healthKitPromptDescription")

            Spacer()

            VStack(spacing: 12) {
                Button(action: onConnect) {
                    Text("Connect")
                        .fontWeight(.semibold)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 14)
                }
                .background(OPColor.terracotta)
                .foregroundStyle(.white)
                .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                .accessibilityIdentifier("healthKitConnectButton")

                Button(action: onDismiss) {
                    Text("Not Now")
                        .fontWeight(.medium)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 14)
                }
                .foregroundStyle(.secondary)
                .accessibilityIdentifier("healthKitDismissButton")
            }
            .padding(.horizontal, 32)
            .padding(.bottom, 32)
        }
    }
}
