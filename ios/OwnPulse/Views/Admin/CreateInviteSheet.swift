// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct CreateInviteSheet: View {
    @Environment(AppDependencies.self) private var dependencies
    @Environment(\.dismiss) private var dismiss
    @State private var label = ""
    @State private var hasMaxUses = false
    @State private var maxUses = 10
    @State private var expiryOption: ExpiryOption = .oneDay
    @State private var isCreating = false
    @State private var createdInvite: InviteCode?
    @State private var errorMessage: String?

    let onCreated: (InviteCode) -> Void

    enum ExpiryOption: String, CaseIterable, Identifiable {
        case oneHour = "1 hour"
        case oneDay = "24 hours"
        case threeDays = "3 days"
        case oneWeek = "1 week"
        case never = "Never"

        var id: String { rawValue }

        var hours: Int? {
            switch self {
            case .oneHour: 1
            case .oneDay: 24
            case .threeDays: 72
            case .oneWeek: 168
            case .never: nil
            }
        }
    }

    var body: some View {
        NavigationStack {
            if let invite = createdInvite {
                inviteCreatedView(invite)
            } else {
                createForm
            }
        }
    }

    private var createForm: some View {
        Form {
            Section("Details") {
                TextField("Label (optional)", text: $label)
                    .accessibilityIdentifier("inviteLabelField")
            }

            Section("Limits") {
                Toggle("Limit uses", isOn: $hasMaxUses)
                    .accessibilityIdentifier("limitUsesToggle")

                if hasMaxUses {
                    Stepper("Max uses: \(maxUses)", value: $maxUses, in: 1...1000)
                        .accessibilityIdentifier("maxUsesStepper")
                }

                Picker("Expires", selection: $expiryOption) {
                    ForEach(ExpiryOption.allCases) { option in
                        Text(option.rawValue).tag(option)
                    }
                }
                .accessibilityIdentifier("expiryPicker")
            }

            if let errorMessage {
                Section {
                    Text(errorMessage)
                        .foregroundStyle(.red)
                        .font(.caption)
                        .accessibilityIdentifier("createInviteError")
                }
            }
        }
        .navigationTitle("Create Invite")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("Cancel") {
                    dismiss()
                }
                .accessibilityIdentifier("cancelCreateInvite")
            }
            ToolbarItem(placement: .confirmationAction) {
                Button("Create") {
                    Task {
                        await createInvite()
                    }
                }
                .disabled(isCreating)
                .accessibilityIdentifier("confirmCreateInvite")
            }
        }
    }

    @ViewBuilder
    private func inviteCreatedView(_ invite: InviteCode) -> some View {
        VStack(spacing: 24) {
            Spacer()

            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 60))
                .foregroundStyle(.green)

            Text("Invite Created")
                .font(.title2)
                .fontWeight(.semibold)

            Text(invite.code)
                .font(.system(.title, design: .monospaced))
                .padding()
                .background(.secondary.opacity(0.1))
                .clipShape(RoundedRectangle(cornerRadius: 8))
                .accessibilityIdentifier("createdInviteCode")

            let inviteURL = inviteURL(for: invite.code)

            ShareLink(
                item: inviteURL,
                subject: Text("OwnPulse Invite"),
                message: Text("Use this link to join OwnPulse: \(inviteURL.absoluteString)")
            ) {
                Label("Share Invite Link", systemImage: "square.and.arrow.up")
                    .frame(maxWidth: .infinity)
                    .padding()
                    .background(.blue)
                    .foregroundStyle(.white)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            .accessibilityIdentifier("shareInviteLink")
            .padding(.horizontal)

            Button("Done") {
                dismiss()
            }
            .accessibilityIdentifier("doneCreateInvite")

            Spacer()
        }
        .navigationTitle("Invite Created")
        .navigationBarTitleDisplayMode(.inline)
    }

    private func inviteURL(for code: String) -> URL {
        var components = URLComponents(
            url: AppConfig.webDashboardURL,
            resolvingAgainstBaseURL: false
        )!
        components.path = "/register"
        components.queryItems = [URLQueryItem(name: "invite", value: code)]
        return components.url!
    }

    private func createInvite() async {
        isCreating = true
        errorMessage = nil

        do {
            let invite = try await dependencies.adminService.createInvite(
                label: label.isEmpty ? nil : label,
                maxUses: hasMaxUses ? maxUses : nil,
                expiresInHours: expiryOption.hours
            )
            createdInvite = invite
            onCreated(invite)
        } catch {
            errorMessage = "Failed to create invite: \(error.localizedDescription)"
        }

        isCreating = false
    }
}
