// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import SwiftUI

struct AdminView: View {
    @Environment(AppDependencies.self) private var dependencies
    @State private var users: [AdminUser] = []
    @State private var invites: [InviteCode] = []
    @State private var isLoading = false
    @State private var errorMessage: String?
    @State private var showCreateInvite = false
    @State private var userToDelete: AdminUser?
    @State private var showDeleteConfirmation = false

    private var currentUserId: String? {
        guard let tokenData = try? dependencies.keychainService.load(
            key: AuthService.accessTokenKey
        ),
            let token = String(data: tokenData, encoding: .utf8),
            let payload = JWTDecoder.decode(token)
        else {
            return nil
        }
        return payload.sub
    }

    var body: some View {
        List {
            if let errorMessage {
                Section {
                    Text(errorMessage)
                        .foregroundStyle(.red)
                        .font(.caption)
                        .accessibilityIdentifier("adminErrorMessage")
                }
            }

            Section("Users") {
                if users.isEmpty && isLoading {
                    ProgressView()
                        .accessibilityIdentifier("usersLoadingIndicator")
                } else {
                    ForEach(users) { user in
                        userRow(user)
                    }
                }
            }

            Section("Invites") {
                if invites.isEmpty && isLoading {
                    ProgressView()
                        .accessibilityIdentifier("invitesLoadingIndicator")
                } else {
                    ForEach(invites) { invite in
                        inviteRow(invite)
                    }
                }
            }
        }
        .navigationTitle("User Management")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    showCreateInvite = true
                } label: {
                    Image(systemName: "plus")
                }
                .accessibilityIdentifier("createInviteButton")
            }
        }
        .sheet(isPresented: $showCreateInvite) {
            CreateInviteSheet { invite in
                invites.insert(invite, at: 0)
            }
        }
        .alert(
            "Delete User",
            isPresented: $showDeleteConfirmation,
            presenting: userToDelete
        ) { user in
            Button("Delete", role: .destructive) {
                Task {
                    await deleteUser(user)
                }
            }
            Button("Cancel", role: .cancel) {}
        } message: { user in
            Text("Are you sure you want to delete \(user.username)? This action cannot be undone.")
        }
        .task {
            await loadData()
        }
        .refreshable {
            await loadData()
        }
    }

    @ViewBuilder
    private func userRow(_ user: AdminUser) -> some View {
        let isCurrentUser = user.id == currentUserId

        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(user.username)
                    .font(.headline)
                Spacer()
                statusBadge(user.status)
            }

            if let email = user.email {
                Text(email)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            if !isCurrentUser {
                Picker("Role", selection: Binding(
                    get: { user.role },
                    set: { newRole in
                        Task {
                            await updateRole(userId: user.id, role: newRole)
                        }
                    }
                )) {
                    Text("Admin").tag("admin")
                    Text("User").tag("user")
                }
                .pickerStyle(.segmented)
                .accessibilityIdentifier("rolePicker_\(user.id)")
            } else {
                Text("Role: \(user.role)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .accessibilityIdentifier("userRow_\(user.id)")
        .swipeActions(edge: .leading) {
            if !isCurrentUser {
                Button {
                    Task {
                        let newStatus = user.status == "active" ? "disabled" : "active"
                        await updateStatus(userId: user.id, status: newStatus)
                    }
                } label: {
                    Text(user.status == "active" ? "Disable" : "Enable")
                }
                .tint(user.status == "active" ? .orange : .green)
            }
        }
        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
            if !isCurrentUser {
                Button(role: .destructive) {
                    userToDelete = user
                    showDeleteConfirmation = true
                } label: {
                    Text("Delete")
                }
            }
        }
    }

    @ViewBuilder
    private func inviteRow(_ invite: InviteCode) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(invite.code)
                    .font(.system(.body, design: .monospaced))
                Spacer()
                if invite.isActive {
                    Text("Active")
                        .font(.caption)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(.green.opacity(0.2))
                        .foregroundStyle(.green)
                        .clipShape(Capsule())
                } else {
                    Text("Revoked")
                        .font(.caption)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(.red.opacity(0.2))
                        .foregroundStyle(.red)
                        .clipShape(Capsule())
                }
            }

            if let label = invite.label {
                Text(label)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            HStack {
                if let maxUses = invite.maxUses {
                    Text("Uses: \(invite.useCount)/\(maxUses)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                } else {
                    Text("Uses: \(invite.useCount)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                if let expiresAt = invite.expiresAt {
                    Text("Expires: \(expiresAt, format: .dateTime)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .accessibilityIdentifier("inviteRow_\(invite.id)")
        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
            if invite.isActive {
                Button(role: .destructive) {
                    Task {
                        await revokeInvite(id: invite.id)
                    }
                } label: {
                    Text("Revoke")
                }
            }
        }
    }

    @ViewBuilder
    private func statusBadge(_ status: String) -> some View {
        Text(status.capitalized)
            .font(.caption)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(status == "active" ? .green.opacity(0.2) : .red.opacity(0.2))
            .foregroundStyle(status == "active" ? .green : .red)
            .clipShape(Capsule())
    }

    private func loadData() async {
        isLoading = true
        errorMessage = nil

        do {
            async let fetchedUsers = dependencies.adminService.listUsers()
            async let fetchedInvites = dependencies.adminService.listInvites()
            users = try await fetchedUsers
            invites = try await fetchedInvites
        } catch {
            errorMessage = "Failed to load data: \(error.localizedDescription)"
        }

        isLoading = false
    }

    private func updateRole(userId: String, role: String) async {
        do {
            let updated = try await dependencies.adminService.updateRole(
                userId: userId, role: role
            )
            if let index = users.firstIndex(where: { $0.id == userId }) {
                users[index] = updated
            }
        } catch {
            errorMessage = "Failed to update role: \(error.localizedDescription)"
        }
    }

    private func updateStatus(userId: String, status: String) async {
        do {
            let updated = try await dependencies.adminService.updateStatus(
                userId: userId, status: status
            )
            if let index = users.firstIndex(where: { $0.id == userId }) {
                users[index] = updated
            }
        } catch {
            errorMessage = "Failed to update status: \(error.localizedDescription)"
        }
    }

    private func deleteUser(_ user: AdminUser) async {
        do {
            try await dependencies.adminService.deleteUser(userId: user.id)
            users.removeAll { $0.id == user.id }
        } catch {
            errorMessage = "Failed to delete user: \(error.localizedDescription)"
        }
    }

    private func revokeInvite(id: String) async {
        do {
            let updated = try await dependencies.adminService.revokeInvite(id: id)
            if let index = invites.firstIndex(where: { $0.id == id }) {
                invites[index] = updated
            }
        } catch {
            errorMessage = "Failed to revoke invite: \(error.localizedDescription)"
        }
    }
}
