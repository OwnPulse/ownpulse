// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { adminApi, AdminUser, InviteCode } from "../api/admin";
import { useAuthStore } from "../store/auth";

function decodeJwtSub(token: string | null): string | null {
  if (!token) return null;
  try {
    const payload = JSON.parse(atob(token.split(".")[1].replace(/-/g, "+").replace(/_/g, "/")));
    return payload.sub ?? null;
  } catch { return null; }
}

const tableStyle: React.CSSProperties = { width: "100%", borderCollapse: "collapse", fontSize: "0.875rem" };
const thStyle: React.CSSProperties = { textAlign: "left", padding: "0.75rem", borderBottom: "2px solid var(--color-border)", color: "var(--color-text-muted)", fontWeight: 600, fontSize: "0.8125rem" };
const tdStyle: React.CSSProperties = { padding: "0.75rem", borderBottom: "1px solid var(--color-border)", color: "var(--color-text)" };
const sectionStyle: React.CSSProperties = { background: "var(--color-surface)", borderRadius: "var(--radius-md)", border: "1px solid var(--color-border)", overflow: "hidden", marginBottom: "2rem" };
const btnStyle: React.CSSProperties = { padding: "0.25rem 0.5rem", borderRadius: "var(--radius-sm)", border: "1px solid var(--color-border)", fontSize: "0.75rem", fontFamily: "var(--font-family)", cursor: "pointer", background: "var(--color-surface)" };

function statusBadge(status: string): React.ReactNode {
  const color = status === "active" ? "#22c55e" : "#ef4444";
  return <span style={{ padding: "0.125rem 0.5rem", borderRadius: "9999px", fontSize: "0.75rem", background: `${color}20`, color }}>{status}</span>;
}

function UsersSection() {
  const queryClient = useQueryClient();
  const currentUserId = decodeJwtSub(useAuthStore((s) => s.token));
  const { data: users, isLoading, isError } = useQuery({ queryKey: ["admin-users"], queryFn: adminApi.listUsers });
  const roleMutation = useMutation({
    mutationFn: ({ userId, role }: { userId: string; role: string }) => adminApi.updateRole(userId, role),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["admin-users"] }),
  });
  const statusMutation = useMutation({
    mutationFn: ({ userId, status }: { userId: string; status: string }) => adminApi.updateUserStatus(userId, status),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["admin-users"] }),
  });
  const deleteMutation = useMutation({
    mutationFn: (userId: string) => adminApi.deleteUser(userId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["admin-users"] }),
  });

  if (isLoading) return <p style={{ padding: "1rem" }}>Loading users...</p>;
  if (isError) return <p style={{ padding: "1rem" }}>Error loading users.</p>;

  return (
    <div style={sectionStyle}>
      <table style={tableStyle}>
        <thead>
          <tr>
            <th style={thStyle}>Email</th>
            <th style={thStyle}>Provider</th>
            <th style={thStyle}>Role</th>
            <th style={thStyle}>Status</th>
            <th style={thStyle}>Created</th>
            <th style={thStyle}>Actions</th>
          </tr>
        </thead>
        <tbody>
          {users?.map((u: AdminUser) => {
            const isSelf = u.id === currentUserId;
            return (
              <tr key={u.id}>
                <td style={tdStyle}>
                  {u.email}
                  {u.username && <span style={{ display: "block", fontSize: "0.75rem", color: "var(--color-text-muted)" }}>{u.username}</span>}
                </td>
                <td style={tdStyle}>{u.auth_provider}</td>
                <td style={tdStyle}>
                  <select value={u.role} disabled={isSelf}
                    onChange={(e) => roleMutation.mutate({ userId: u.id, role: e.target.value })}
                    style={{ padding: "0.25rem 0.5rem", borderRadius: "var(--radius-sm)", border: "1px solid var(--color-border)", fontSize: "0.8125rem", fontFamily: "var(--font-family)" }}>
                    <option value="admin">admin</option>
                    <option value="user">user</option>
                  </select>
                </td>
                <td style={tdStyle}>{statusBadge(u.status)}</td>
                <td style={tdStyle}>{new Date(u.created_at).toLocaleDateString()}</td>
                <td style={tdStyle}>
                  {!isSelf && (
                    <span style={{ display: "flex", gap: "0.5rem" }}>
                      <button style={btnStyle}
                        onClick={() => statusMutation.mutate({ userId: u.id, status: u.status === "active" ? "disabled" : "active" })}>
                        {u.status === "active" ? "Disable" : "Enable"}
                      </button>
                      <button style={{ ...btnStyle, color: "#ef4444", borderColor: "#ef4444" }}
                        onClick={() => { if (window.confirm(`Delete user ${u.email}? This cannot be undone.`)) deleteMutation.mutate(u.id); }}>
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
  const { data: invites, isLoading } = useQuery({ queryKey: ["admin-invites"], queryFn: adminApi.listInvites });
  const [showForm, setShowForm] = useState(false);
  const [label, setLabel] = useState("");
  const [maxUses, setMaxUses] = useState("");
  const [expiresInHours, setExpiresInHours] = useState("");
  const [copied, setCopied] = useState<string | null>(null);

  const createMutation = useMutation({
    mutationFn: (data: { label?: string; max_uses?: number; expires_in_hours?: number }) => adminApi.createInvite(data),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ["admin-invites"] }); setShowForm(false); setLabel(""); setMaxUses(""); setExpiresInHours(""); },
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
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1rem" }}>
        <h2 style={{ fontSize: "1.25rem", color: "var(--color-text)" }}>Invites</h2>
        <button style={{ ...btnStyle, background: "var(--color-primary)", color: "#fff", border: "none", padding: "0.375rem 0.75rem" }}
          onClick={() => setShowForm(!showForm)}>
          {showForm ? "Cancel" : "Create Invite"}
        </button>
      </div>

      {showForm && (
        <form onSubmit={handleCreate} style={{ ...sectionStyle, padding: "1rem", display: "flex", gap: "0.75rem", flexWrap: "wrap", alignItems: "end" }}>
          <div>
            <label style={{ display: "block", fontSize: "0.75rem", color: "var(--color-text-muted)", marginBottom: "0.25rem" }}>Label</label>
            <input type="text" value={label} onChange={(e) => setLabel(e.target.value)} placeholder="e.g. for Alice"
              style={{ padding: "0.375rem 0.5rem", borderRadius: "var(--radius-sm)", border: "1px solid var(--color-border)", fontSize: "0.8125rem" }} />
          </div>
          <div>
            <label style={{ display: "block", fontSize: "0.75rem", color: "var(--color-text-muted)", marginBottom: "0.25rem" }}>Max uses</label>
            <input type="number" value={maxUses} onChange={(e) => setMaxUses(e.target.value)} min="1" placeholder="unlimited"
              style={{ padding: "0.375rem 0.5rem", borderRadius: "var(--radius-sm)", border: "1px solid var(--color-border)", fontSize: "0.8125rem", width: "6rem" }} />
          </div>
          <div>
            <label style={{ display: "block", fontSize: "0.75rem", color: "var(--color-text-muted)", marginBottom: "0.25rem" }}>Expires in (hours)</label>
            <input type="number" value={expiresInHours} onChange={(e) => setExpiresInHours(e.target.value)} min="1" placeholder="never"
              style={{ padding: "0.375rem 0.5rem", borderRadius: "var(--radius-sm)", border: "1px solid var(--color-border)", fontSize: "0.8125rem", width: "6rem" }} />
          </div>
          <button type="submit" disabled={createMutation.isPending}
            style={{ ...btnStyle, background: "var(--color-primary)", color: "#fff", border: "none", padding: "0.375rem 0.75rem" }}>
            Create
          </button>
        </form>
      )}

      <div style={sectionStyle}>
        {isLoading ? <p style={{ padding: "1rem" }}>Loading invites...</p> : (
          <table style={tableStyle}>
            <thead>
              <tr>
                <th style={thStyle}>Code</th>
                <th style={thStyle}>Label</th>
                <th style={thStyle}>Uses</th>
                <th style={thStyle}>Expires</th>
                <th style={thStyle}>Status</th>
                <th style={thStyle}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {invites?.map((inv: InviteCode) => (
                <tr key={inv.id}>
                  <td style={{ ...tdStyle, fontFamily: "monospace" }}>{inv.code}</td>
                  <td style={tdStyle}>{inv.label || "\u2014"}</td>
                  <td style={tdStyle}>{inv.use_count}/{inv.max_uses ?? "\u221E"}</td>
                  <td style={tdStyle}>{inv.expires_at ? new Date(inv.expires_at).toLocaleString() : "Never"}</td>
                  <td style={tdStyle}>{statusBadge(inviteStatus(inv) === "Active" ? "active" : "disabled")}</td>
                  <td style={tdStyle}>
                    <span style={{ display: "flex", gap: "0.5rem" }}>
                      <button style={btnStyle} onClick={() => copyUrl(inv.code)}>
                        {copied === inv.code ? "Copied!" : "Copy URL"}
                      </button>
                      {inviteStatus(inv) === "Active" && (
                        <button style={{ ...btnStyle, color: "#ef4444" }} onClick={() => revokeMutation.mutate(inv.id)}>Revoke</button>
                      )}
                    </span>
                  </td>
                </tr>
              ))}
              {invites?.length === 0 && (
                <tr><td colSpan={6} style={{ ...tdStyle, textAlign: "center", color: "var(--color-text-muted)" }}>No invites yet.</td></tr>
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
    <main style={{ padding: "1.5rem", fontFamily: "var(--font-family)" }}>
      <h1 style={{ fontSize: "1.5rem", color: "var(--color-text)", marginBottom: "1.5rem" }}>Admin</h1>
      <h2 style={{ fontSize: "1.25rem", color: "var(--color-text)", marginBottom: "1rem" }}>Users</h2>
      <UsersSection />
      <InvitesSection />
    </main>
  );
}
