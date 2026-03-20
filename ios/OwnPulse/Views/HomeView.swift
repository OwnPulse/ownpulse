// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct HomeView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var isSyncing = false
    @State private var lastSyncDate: Date?
    @State private var syncError: String?

    var body: some View {
        NavigationStack {
            VStack(spacing: 20) {
                // Sync status
                GroupBox("Sync Status") {
                    VStack(alignment: .leading, spacing: 8) {
                        if isSyncing {
                            HStack {
                                ProgressView()
                                Text("Syncing...")
                            }
                            .accessibilityIdentifier("syncingIndicator")
                        } else if let lastSync = lastSyncDate {
                            Text("Last synced: \(lastSync, format: .relative(presentation: .named))")
                                .accessibilityIdentifier("lastSyncLabel")
                        } else {
                            Text("Not yet synced")
                                .foregroundStyle(.secondary)
                                .accessibilityIdentifier("notSyncedLabel")
                        }

                        if let error = syncError {
                            Text(error)
                                .foregroundStyle(.red)
                                .font(.caption)
                                .accessibilityIdentifier("syncError")
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }

                // Sync Now button
                Button {
                    Task {
                        await performSync()
                    }
                } label: {
                    Text("Sync Now")
                        .frame(maxWidth: .infinity)
                        .padding()
                        .background(.blue)
                        .foregroundStyle(.white)
                        .clipShape(RoundedRectangle(cornerRadius: 12))
                }
                .disabled(isSyncing)
                .accessibilityIdentifier("syncNowButton")

                // Open Dashboard
                Link(destination: AppConfig.webDashboardURL) {
                    Text("Open Dashboard")
                        .frame(maxWidth: .infinity)
                        .padding()
                        .background(.secondary.opacity(0.2))
                        .clipShape(RoundedRectangle(cornerRadius: 12))
                }
                .accessibilityIdentifier("openDashboardButton")

                Spacer()
            }
            .padding()
            .navigationTitle("OwnPulse")
            .toolbar {
                NavigationLink {
                    SettingsView()
                } label: {
                    Image(systemName: "gear")
                }
                .accessibilityIdentifier("settingsButton")
            }
            .task {
                await performSync()
            }
        }
    }

    private func performSync() async {
        isSyncing = true
        syncError = nil
        await dependencies.syncEngine.sync()
        lastSyncDate = await dependencies.syncEngine.lastSyncDate
        syncError = await dependencies.syncEngine.lastError
        isSyncing = false
        dependencies.syncScheduler.scheduleNextSync()
    }
}
