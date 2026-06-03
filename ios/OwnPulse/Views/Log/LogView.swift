// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct LogView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var viewModel: LogViewModel?

    var body: some View {
        ScrollView {
            if let vm = viewModel {
                VStack(spacing: 20) {
                    // Tab selector — scrollable chip row (too many tabs for a
                    // segmented control to remain tappable).
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 8) {
                            ForEach(LogTab.allCases, id: \.self) { tab in
                                tabChip(tab, isSelected: vm.selectedTab == tab) {
                                    vm.selectedTab = tab
                                }
                            }
                        }
                    }
                    .accessibilityIdentifier("logTabPicker")

                    // Form content
                    switch vm.selectedTab {
                    case .checkin:
                        CheckinForm(viewModel: vm)
                    case .intervention:
                        InterventionForm(viewModel: vm)
                    case .observation:
                        ObservationForm(viewModel: vm)
                    case .weight:
                        WeightEntryForm(viewModel: vm)
                    case .sleep:
                        SleepEntryForm(viewModel: vm)
                    case .exercise:
                        ExerciseEntryForm(viewModel: vm)
                    case .glucose:
                        GlucoseEntryForm(viewModel: vm)
                    case .bloodPressure:
                        BloodPressureEntryForm(viewModel: vm)
                    }

                    // Status message
                    statusMessage(vm: vm)
                }
                .padding(16)
            }
        }
        .scrollDismissesKeyboard(.interactively)
        .navigationTitle("Log")
        .onAppear {
            if viewModel == nil {
                viewModel = LogViewModel(networkClient: dependencies.networkClient)
            }
        }
    }

    private func tabChip(_ tab: LogTab, isSelected: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(tab.rawValue)
                .font(.subheadline)
                .fontWeight(isSelected ? .semibold : .regular)
                .padding(.horizontal, 14)
                .padding(.vertical, 8)
                .background(
                    Capsule()
                        .fill(isSelected ? OPColor.teal : Color(.secondarySystemBackground))
                )
                .foregroundStyle(isSelected ? .white : .primary)
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("logTab-\(tab.rawValue)")
    }

    @ViewBuilder
    private func statusMessage(vm: LogViewModel) -> some View {
        switch vm.submitState {
        case .success(let message):
            HStack {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundStyle(OPColor.sage)
                Text(message)
                    .foregroundStyle(OPColor.sage)
            }
            .font(.subheadline)
            .fontWeight(.medium)
            .transition(.move(edge: .bottom).combined(with: .opacity))
            .accessibilityIdentifier("submitSuccess")

        case .error(let message):
            HStack {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(.red)
                Text(message)
                    .foregroundStyle(.red)
            }
            .font(.caption)
            .transition(.move(edge: .bottom).combined(with: .opacity))
            .accessibilityIdentifier("submitError")

        case .idle, .submitting:
            EmptyView()
        }
    }
}
