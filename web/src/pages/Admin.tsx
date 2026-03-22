// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type AdminUser, adminApi, type InviteCode } from "../api/admin";
import { useAuthStore } from "../store/auth";
import styles from "./Admin.module.css";

function decodeJwtSub(token: string | null): string | null {
  if (!token) return null;
  try {
    const payload = JSON.parse(atob(token.split(".")[1].replace(/-/g, "+").replace(/_/g, "/")));
    return payload.sub ?? null;
  } catch {
    return null;
  }
}

function statusBadge(status: string): React.ReactNode {
  const isActive = status === "active";
  return (
    <span className={`op-badge ${isActive ? "op-badge-success" : "op-badge-error"}`}>{status}</span>
  );
}

function UsersSection() {
  const queryClient = useQueryClient();
  const currentUserId = decodeJwtSub(useAuthStore((s) => s.token));
  const {
    data: users,
    isLoading,
    isError,
  } = useQuery({ queryKey: ["admin-users"], queryFn: adminApi.listUsers });
  const roleMutation = useMutation({
    mutationFn: ({ userId, role }: { userId: string; role: string }) =>
      adminApi.updateRole(userId, role),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["admin-users"] }),
  });
  const statusMutation = useMutation({
    mutationFn: ({ userId, status }: { userId: string; status: string }) =>
      adminApi.updateUserStatus(userId, status),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["admin-users"] }),
  });
  const deleteMutation = useMutation({
    mutationFn: (userId: string) => adminApi.deleteUser(userId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["admin-users"] }),
  });

  if (isLoading) return <p>Loading users...</p>;
  if (isError) return <p>Error loading users.</p>;

  return (
    <div className={styles.section}>
      <table className="op-table">
        <thead>
          <tr>
            <th>Email</th>
            <th>Provider</th>
            <th>Role</th>
            <th>Status</th>
            <th>Created</th>
            <th>Actions</th>
          </tr>
        </thead>
        <tbody>
          {users?.map((u: AdminUser) => {
            const isSelf = u.id === currentUserId;
            return (
              <tr key={u.id}>
                <td>
                  {u.email}
                  {u.username && <span className={styles.username}>{u.username}</span>}
                </td>
                <td>{u.auth_provider}</td>
                <td>
                  <select
                    value={u.role}
                    disabled={isSelf}
                    onChange={(e) => roleMutation.mutate({ userId: u.id, role: e.target.value })}
                    className={styles.roleSelect}
                  >
                    <option value="admin">admin</option>
                    <option value="user">user</option>
                  </select>
                </td>
                <td>{statusBadge(u.status)}</td>
                <td>{new Date(u.created_at).toLocaleDateString()}</td>
                <td>
                  {!isSelf && (
                    <span className={styles.actions}>
                      <button
                        type="button"
                        className="op-btn op-btn-ghost op-btn-sm"
                        onClick={() =>
                          statusMutation.mutate({
                            userId: u.id,
                            status: u.status === "active" ? "disabled" : "active",
                          })
                        }
                      >
                        {u.status === "active" ? "Disable" : "Enable"}
                      </button>
                      <button
                        type="button"
                        className="op-btn op-btn-danger op-btn-sm"
                        onClick={() => {
                          if (window.confirm(`Delete user ${u.email}? This cannot be undone.`))
                            deleteMutation.mutate(u.id);
                        }}
                      >
                        Delete
                      </button>
                    </span>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function InvitesSection() {
  const queryClient = useQueryClient();
  const { data: invites, isLoading } = useQuery({
    queryKey: ["admin-invites"],
    queryFn: adminApi.listInvites,
  });
  const [showForm, setShowForm] = useState(false);
  const [label, setLabel] = useState("");
  const [maxUses, setMaxUses] = useState("");
  const [expiresInHours, setExpiresInHours] = useState("");
  const [copied, setCopied] = useState<string | null>(null);

  const createMutation = useMutation({
    mutationFn: (data: { label?: string; max_uses?: number; expires_in_hours?: number }) =>
      adminApi.createInvite(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["admin-invites"] });
      setShowForm(false);
      setLabel("");
      setMaxUses("");
      setExpiresInHours("");
    },
  });
  const revokeMutation = useMutation({
    mutationFn: (id: string) => adminApi.revokeInvite(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["admin-invites"] }),
  });

  const handleCreate = (e: React.FormEvent) => {
    e.preventDefault();
    createMutation.mutate({
      label: label || undefined,
      max_uses: maxUses ? parseInt(maxUses, 10) : undefined,
      expires_in_hours: expiresInHours ? parseInt(expiresInHours, 10) : undefined,
    });
  };

  const copyUrl = (code: string) => {
    navigator.clipboard.writeText(`${window.location.origin}/register?invite=${code}`);
    setCopied(code);
    setTimeout(() => setCopied(null), 2000);
  };

  const inviteStatus = (inv: InviteCode): string => {
    if (inv.revoked_at) return "Revoked";
    if (inv.expires_at && new Date(inv.expires_at) < new Date()) return "Expired";
    return "Active";
  };

  return (
    <div>
      <div className={styles.inviteHeader}>
        <h2>Invites</h2>
        <button
          type="button"
          className="op-btn op-btn-primary op-btn-sm"
          onClick={() => setShowForm(!showForm)}
        >
          {showForm ? "Cancel" : "Create Invite"}
        </button>
      </div>

      {showForm && (
        <form onSubmit={handleCreate} className={styles.createForm}>
          <div className={styles.createFormField}>
            <label htmlFor="invite-label" className={styles.createFormLabel}>
              Label
            </label>
            <input
              id="invite-label"
              type="text"
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              placeholder="e.g. for Alice"
              className={styles.createFormInput}
            />
          </div>
          <div className={styles.createFormField}>
            <label htmlFor="invite-max-uses" className={styles.createFormLabel}>
              Max uses
            </label>
            <input
              id="invite-max-uses"
              type="number"
              value={maxUses}
              onChange={(e) => setMaxUses(e.target.value)}
              min="1"
              placeholder="unlimited"
              className={styles.createFormInputNarrow}
            />
          </div>
          <div className={styles.createFormField}>
            <label htmlFor="invite-expires" className={styles.createFormLabel}>
              Expires in (hours)
            </label>
            <input
              id="invite-expires"
              type="number"
              value={expiresInHours}
              onChange={(e) => setExpiresInHours(e.target.value)}
              min="1"
              placeholder="never"
              className={styles.createFormInputNarrow}
            />
          </div>
          <button
            type="submit"
            disabled={createMutation.isPending}
            className="op-btn op-btn-primary op-btn-sm"
          >
            Create
          </button>
        </form>
      )}

      <div className={styles.section}>
        {isLoading ? (
          <p className={styles.loadingText}>Loading invites...</p>
        ) : (
          <table className="op-table">
            <thead>
              <tr>
                <th>Code</th>
                <th>Label</th>
                <th>Uses</th>
                <th>Expires</th>
                <th>Status</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {invites?.map((inv: InviteCode) => (
                <tr key={inv.id}>
                  <td className={styles.codeCell}>{inv.code}</td>
                  <td>{inv.label || "\u2014"}</td>
                  <td>
                    {inv.use_count}/{inv.max_uses ?? "\u221E"}
                  </td>
                  <td>{inv.expires_at ? new Date(inv.expires_at).toLocaleString() : "Never"}</td>
                  <td>{statusBadge(inviteStatus(inv) === "Active" ? "active" : "disabled")}</td>
                  <td>
                    <span className={styles.actions}>
                      <button
                        type="button"
                        className="op-btn op-btn-ghost op-btn-sm"
                        onClick={() => copyUrl(inv.code)}
                      >
                        {copied === inv.code ? "Copied!" : "Copy URL"}
                      </button>
                      {inviteStatus(inv) === "Active" && (
                        <button
                          type="button"
                          className="op-btn op-btn-danger op-btn-sm"
                          onClick={() => revokeMutation.mutate(inv.id)}
                        >
                          Revoke
                        </button>
                      )}
                    </span>
                  </td>
                </tr>
              ))}
              {invites?.length === 0 && (
                <tr>
                  <td colSpan={6} className="op-empty">
                    No invites yet.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}

export default function Admin() {
  return (
    <main className="op-page">
      <h1>Admin</h1>
      <h2>Users</h2>
      <UsersSection />
      <InvitesSection />
    </main>
  );
}
