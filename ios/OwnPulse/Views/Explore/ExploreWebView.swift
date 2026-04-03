// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct ExploreWebView: View {
    var body: some View {
        VStack(spacing: 24) {
            Spacer()

            Image(systemName: "chart.xyaxis.line")
                .font(.system(size: 64))
                .foregroundStyle(.secondary)

            Text("Charts Coming Soon")
                .font(.title2)
                .fontWeight(.semibold)

            Text("Native charts are being built. In the meantime, you can explore your data on the web.")
                .font(.body)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)

            Button {
                if let url = URL(string: "\(AppConfig.webDashboardURL)/explore") {
                    UIApplication.shared.open(url)
                }
            } label: {
                Label("Open in Browser", systemImage: "safari")
                    .font(.body.weight(.medium))
                    .padding(.horizontal, 24)
                    .padding(.vertical, 12)
                    .background(.blue)
                    .foregroundStyle(.white)
                    .clipShape(RoundedRectangle(cornerRadius: 10))
            }

            Spacer()
        }
        .navigationTitle("Explore")
        .navigationBarTitleDisplayMode(.inline)
    }
}
