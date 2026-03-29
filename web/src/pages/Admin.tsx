// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type AdminUser, adminApi, type InviteCode } from "../api/admin";
import { type InviteClaim, invitesApi } from "../api/invites";
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

function inviteStatusLabel(inv: InviteCode): "Active" | "Revoked" | "Expired" {
  if (inv.revoked_at) return "Revoked";
  if (inv.expires_at && new Date(inv.expires_at) < new Date()) return "Expired";
  return "Active";
}

function statusBadgeClass(status: "Active" | "Revoked" | "Expired"): string {
  if (status === "Active") return "op-badge op-badge-success";
  if (status === "Revoked") return "op-badge op-badge-error";
  return "op-badge op-badge-error";
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
                <td>
                  <span
                    className={`op-badge ${u.status === "active" ? "op-badge-success" : "op-badge-error"}`}
                  >
                    {u.status}
                  </span>
                </td>
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

function InviteStats({ invites }: { invites: InviteCode[] }) {
  const active = invites.filter((i) => inviteStatusLabel(i) === "Active").length;
  const expired = invites.filter((i) => inviteStatusLabel(i) === "Expired").length;
  const revoked = invites.filter((i) => inviteStatusLabel(i) === "Revoked").length;
  const totalUsed = invites.reduce((sum, i) => sum + i.use_count, 0);

  return (
    <div className={styles.statsSummary}>
      <span className={styles.statItem}>
        <span className={styles.statCount}>{active}</span> active
      </span>
      <span className={styles.statItem}>
        <span className={styles.statCount}>{totalUsed}</span> used
      </span>
      <span className={styles.statItem}>
        <span className={styles.statCount}>{expired}</span> expired
      </span>
      <span className={styles.statItem}>
        <span className={styles.statCount}>{revoked}</span> revoked
      </span>
    </div>
  );
}

function InviteClaimsList({ inviteId }: { inviteId: string }) {
  const { data: claims, isLoading } = useQuery({
    queryKey: ["invite-claims", inviteId],
    queryFn: () => invitesApi.getClaims(inviteId),
  });

  if (isLoading) return <span className={styles.claimsLoading}>Loading...</span>;
  if (!claims || claims.length === 0) return null;

  return (
    <ul className={styles.claimsList}>
      {claims.map((c: InviteClaim) => (
        <li key={c.user_email} className={styles.claimItem}>
          {c.user_email}{" "}
          <span className={styles.claimDate}>{new Date(c.claimed_at).toLocaleDateString()}</span>
        </li>
      ))}
    </ul>
  );
}

function UsageBar({ used, max }: { used: number; max: number }) {
  const pct = Math.min(100, (used / max) * 100);
  return (
    <div className={styles.usageBarWrap}>
      <div className={styles.usageBar}>
        <div
          className={styles.usageBarFill}
          style={{ width: `${pct}%` }}
          role="progressbar"
          aria-valuenow={used}
          aria-valuemin={0}
          aria-valuemax={max}
        />
      </div>
      <span className={styles.usageText}>
        {used}/{max}
      </span>
    </div>
  );
}

function InviteShareUrl({ code }: { code: string }) {
  const [copied, setCopied] = useState(false);
  const url = `${window.location.origin}/invite/${code}`;

  const handleCopy = () => {
    navigator.clipboard.writeText(url);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className={styles.shareUrlWrap}>
      <code className={styles.shareUrl}>{url}</code>
      <button type="button" className="op-btn op-btn-ghost op-btn-sm" onClick={handleCopy}>
        {copied ? "Copied!" : "Copy Link"}
      </button>
    </div>
  );
}

function SendEmailForm({ inviteId }: { inviteId: string }) {
  const [email, setEmail] = useState("");
  const [sent, setSent] = useState(false);

  const mutation = useMutation({
    mutationFn: (emailAddr: string) => invitesApi.sendEmail(inviteId, { email: emailAddr }),
    onSuccess: () => {
      setSent(true);
      setEmail("");
      setTimeout(() => setSent(false), 3000);
    },
  });

  return (
    <div className={styles.sendEmailRow}>
      <input
        type="email"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        placeholder="recipient@example.com"
        className={styles.createFormInput}
        aria-label="Recipient email"
      />
      <button
        type="button"
        className="op-btn op-btn-ghost op-btn-sm"
        disabled={!email || mutation.isPending}
        onClick={() => mutation.mutate(email)}
      >
        {sent ? "Sent!" : "Send Email"}
      </button>
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
  const [sendEmail, setSendEmail] = useState("");
  const [createdInvite, setCreatedInvite] = useState<InviteCode | null>(null);
  const [expandedClaims, setExpandedClaims] = useState<Set<string>>(new Set());

  const createMutation = useMutation({
    mutationFn: (data: { label?: string; max_uses?: number; expires_in_hours?: number }) =>
      adminApi.createInvite(data),
    onSuccess: (invite) => {
      queryClient.invalidateQueries({ queryKey: ["admin-invites"] });
      setCreatedInvite(invite);
      setShowForm(false);
      setLabel("");
      setMaxUses("");
      setExpiresInHours("");
      if (sendEmail) {
        invitesApi.sendEmail(invite.id, { email: sendEmail });
        setSendEmail("");
      }
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

  const toggleClaims = (id: string) => {
    setExpandedClaims((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  return (
    <div>
      <div className={styles.inviteHeader}>
        <h2>Invites</h2>
        <button
          type="button"
          className="op-btn op-btn-primary op-btn-sm"
          onClick={() => {
            setShowForm(!showForm);
            setCreatedInvite(null);
          }}
        >
          {showForm ? "Cancel" : "Create Invite"}
        </button>
      </div>

      {invites && invites.length > 0 && <InviteStats invites={invites} />}

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
          <div className={styles.createFormField}>
            <label htmlFor="invite-send-email" className={styles.createFormLabel}>
              Send via email (optional)
            </label>
            <input
              id="invite-send-email"
              type="email"
              value={sendEmail}
              onChange={(e) => setSendEmail(e.target.value)}
              placeholder="recipient@example.com"
              className={styles.createFormInput}
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

      {createdInvite && (
        <div className={styles.createdInviteCard}>
          <p className={styles.createdInviteTitle}>Invite created</p>
          <InviteShareUrl code={createdInvite.code} />
          <SendEmailForm inviteId={createdInvite.id} />
        </div>
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
              {invites?.map((inv: InviteCode) => {
                const status = inviteStatusLabel(inv);
                return (
                  <tr key={inv.id}>
                    <td className={styles.codeCell}>{inv.code}</td>
                    <td>{inv.label || "\u2014"}</td>
                    <td>
                      {inv.max_uses ? (
                        <UsageBar used={inv.use_count} max={inv.max_uses} />
                      ) : (
                        <span>{inv.use_count}/\u221E</span>
                      )}
                    </td>
                    <td>{inv.expires_at ? new Date(inv.expires_at).toLocaleString() : "Never"}</td>
                    <td>
                      <span className={statusBadgeClass(status)}>{status.toLowerCase()}</span>
                    </td>
                    <td>
                      <span className={styles.actions}>
                        <button
                          type="button"
                          className="op-btn op-btn-ghost op-btn-sm"
                          onClick={() => {
                            navigator.clipboard.writeText(
                              `${window.location.origin}/invite/${inv.code}`,
                            );
                          }}
                        >
                          Copy Link
                        </button>
                        {inv.use_count > 0 && (
                          <button
                            type="button"
                            className="op-btn op-btn-ghost op-btn-sm"
                            onClick={() => toggleClaims(inv.id)}
                          >
                            {expandedClaims.has(inv.id) ? "Hide Claims" : "Show Claims"}
                          </button>
                        )}
                        {status === "Active" && (
                          <button
                            type="button"
                            className="op-btn op-btn-danger op-btn-sm"
                            onClick={() => revokeMutation.mutate(inv.id)}
                          >
                            Revoke
                          </button>
                        )}
                      </span>
                      {expandedClaims.has(inv.id) && <InviteClaimsList inviteId={inv.id} />}
                    </td>
                  </tr>
                );
              })}
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
