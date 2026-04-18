// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct SyncStatusView: View {
    @Environment(AppDependencies.self) private var dependencies

    private var progress: SyncProgress { dependencies.syncProgress }
    private var isSyncing: Bool { progress.currentType != nil }

    private let categories: [(name: String, types: [String])] = [
        ("Vitals", ["heart_rate", "resting_heart_rate", "heart_rate_variability", "blood_pressure_systolic", "blood_pressure_diastolic", "blood_oxygen", "respiratory_rate", "body_temperature", "blood_glucose", "sleeping_wrist_temperature"]),
        ("Body", ["body_mass", "body_fat_percentage", "lean_body_mass", "height", "waist_circumference", "bmi"]),
        ("Activity", ["steps", "distance_walking_running", "flights_climbed", "exercise_time", "stand_time", "move_time", "active_energy", "basal_energy", "swimming_strokes", "physical_effort"]),
        ("Running", ["running_speed", "running_power", "running_stride_length", "running_vertical_oscillation", "running_ground_contact_time"]),
        ("Cycling", ["cycling_speed", "cycling_power", "cycling_cadence", "cycling_ftp", "distance_cycling"]),
        ("Mobility", ["walking_speed", "walking_step_length", "walking_asymmetry", "walking_double_support", "stair_ascent_speed", "stair_descent_speed", "six_min_walk_distance"]),
        ("Sleep", ["sleep_analysis"]),
        ("Dietary", ["dietary_energy", "water_intake", "dietary_protein", "dietary_fat", "dietary_carbs", "dietary_fiber", "dietary_sugar", "dietary_caffeine", "dietary_sodium", "dietary_cholesterol", "dietary_iron", "dietary_vitamin_c", "dietary_vitamin_d", "dietary_calcium", "dietary_potassium", "dietary_zinc", "dietary_magnesium"]),
        ("Environment", ["time_in_daylight", "environmental_audio", "headphone_audio", "falls"]),
        ("Events", ["mindful_session", "high_heart_rate_event", "low_heart_rate_event", "irregular_heart_rhythm_event", "stand_hour"]),
    ]

    @State private var errorDetail: String?

    var body: some View {
        ScrollView {
            LazyVStack(spacing: 16) {
                overallCard
                ForEach(categories, id: \.name) { category in
                    categorySection(category)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
        }
        .navigationTitle("Sync Status")
        .task {
            // Load historical sync timestamps on appear
            if progress.typeStatuses.isEmpty {
                let timestamps = (try? dependencies.anchorStore.allSyncTimestamps()) ?? [:]
                let types = HealthKitTypeMap.mappings.map {
                    (recordType: $0.recordType, displayName: $0.recordType.replacingOccurrences(of: "_", with: " ").capitalized)
                }
                progress.reset(types: types, timestamps: timestamps)
            }
        }
        .refreshable {
            await dependencies.syncEngine.sync()
        }
        .alert("Sync Error", isPresented: .constant(errorDetail != nil)) {
            Button("OK") { errorDetail = nil }
        } message: {
            Text(errorDetail ?? "")
        }
    }

    private var overallCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                if isSyncing {
                    ProgressView()
                        .controlSize(.small)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Syncing \(progress.completedTypes) / \(progress.totalTypes) types")
                            .font(.subheadline)
                        if progress.totalRecordsUploaded > 0 {
                            Text("\(progress.totalRecordsUploaded.formatted()) records uploaded")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .monospacedDigit()
                        }
                    }
                } else {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundStyle(OPColor.sage)
                    if let last = dependencies.syncProgress.typeStatuses.values
                        .compactMap({ $0.lastSyncTime }).max() {
                        Text("Last sync \(last, format: .relative(presentation: .named))")
                            .font(.subheadline)
                    } else {
                        Text("Never synced")
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }
                }
                Spacer()
            }

            if isSyncing && progress.totalTypes > 0 {
                ProgressView(
                    value: Double(progress.completedTypes),
                    total: Double(progress.totalTypes)
                )
                .tint(OPColor.terracotta)
            }

            if !isSyncing {
                Button {
                    Task { await dependencies.syncEngine.sync() }
                } label: {
                    Label("Sync Now", systemImage: "arrow.triangle.2.circlepath")
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 10)
                }
                .buttonStyle(.borderedProminent)
                .tint(OPColor.terracotta)
                .accessibilityIdentifier("syncNowButton")
            }
        }
        .opCard()
    }

    @ViewBuilder
    private func categorySection(_ category: (name: String, types: [String])) -> some View {
        let statuses = category.types.compactMap { progress.typeStatuses[$0] }
        if !statuses.isEmpty {
            VStack(alignment: .leading, spacing: 4) {
                Text(category.name)
                    .font(.headline)
                    .padding(.leading, 4)

                VStack(spacing: 0) {
                    ForEach(statuses, id: \.recordType) { status in
                        typeRow(status)
                        if status.recordType != statuses.last?.recordType {
                            Divider().padding(.leading, 36)
                        }
                    }
                }
                .opCard()
            }
        }
    }

    private func typeRow(_ status: TypeSyncStatus) -> some View {
        Button {
            if let error = status.error {
                errorDetail = "\(status.displayName): \(error)"
            }
        } label: {
            VStack(spacing: 4) {
                HStack(spacing: 10) {
                    statusIcon(status.status)
                        .frame(width: 20)

                    Text(status.displayName)
                        .font(.subheadline)
                        .foregroundStyle(.primary)

                    Spacer()

                    if status.status == .syncing {
                        if status.totalSamples > 0 {
                            Text("\(status.recordsSynced) / \(status.totalSamples)")
                                .font(.caption.monospacedDigit())
                                .foregroundStyle(.secondary)
                        } else {
                            ProgressView()
                                .controlSize(.mini)
                        }
                    } else if let time = status.lastSyncTime {
                        Text(time, format: .relative(presentation: .named))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    } else {
                        Text("never")
                            .font(.caption)
                            .foregroundStyle(.tertiary)
                    }

                    if status.status != .syncing && status.recordsSynced > 0 {
                        Text("\(status.recordsSynced)")
                            .font(.caption.monospacedDigit())
                            .foregroundStyle(.secondary)
                            .frame(minWidth: 30, alignment: .trailing)
                    }
                }

                if status.status == .syncing && status.totalSamples > 0 {
                    ProgressView(
                        value: Double(status.recordsSynced),
                        total: Double(status.totalSamples)
                    )
                    .tint(OPColor.teal)
                    .padding(.leading, 30)
                }
            }
            .padding(.vertical, 6)
        }
        .disabled(status.error == nil)
        .accessibilityIdentifier("syncType-\(status.recordType)")
    }

    @ViewBuilder
    private func statusIcon(_ state: SyncTypeState) -> some View {
        switch state {
        case .synced:
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(OPColor.sage)
        case .skipped:
            Image(systemName: "minus.circle")
                .foregroundStyle(.secondary)
        case .syncing:
            Image(systemName: "arrow.triangle.2.circlepath")
                .foregroundStyle(OPColor.teal)
        case .failed:
            Image(systemName: "xmark.circle.fill")
                .foregroundStyle(.red)
        case .pending:
            Image(systemName: "circle")
                .foregroundStyle(.secondary)
        case .never:
            Image(systemName: "circle")
                .foregroundStyle(.tertiary)
        }
    }
}
