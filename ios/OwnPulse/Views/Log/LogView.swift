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
                    // Segmented control
                    Picker("Log Type", selection: Bindable(vm).selectedTab) {
                        ForEach(LogTab.allCases, id: \.self) { tab in
                            Text(tab.rawValue).tag(tab)
                        }
                    }
                    .pickerStyle(.segmented)
                    .accessibilityIdentifier("logTabPicker")

                    // Form content
                    switch vm.selectedTab {
                    case .checkin:
                        CheckinForm(viewModel: vm)
                    case .intervention:
                        InterventionForm(viewModel: vm)
                    case .observation:
                        ObservationForm(viewModel: vm)
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
