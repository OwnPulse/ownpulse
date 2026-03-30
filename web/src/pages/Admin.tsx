// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type AdminUser, adminApi, type CreateInviteRequest, type InviteCode } from "../api/admin";
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

function inviteStatusLabel(inv: InviteCode): "Active" | "Expired" | "Revoked" {
  if (inv.revoked_at) return "Revoked";
  if (inv.expires_at && new Date(inv.expires_at) < new Date()) return "Expired";
  return "Active";
}

function inviteStatusBadgeClass(status: "Active" | "Expired" | "Revoked"): string {
  if (status === "Active") return "op-badge op-badge-success";
  if (status === "Revoked") return "op-badge op-badge-error";
  return "op-badge op-badge-muted";
}

function inviteLink(code: string): string {
  return `${window.location.origin}/invite/${code}`;
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
    <div className={styles.userGrid}>
      {users?.map((u: AdminUser) => {
        const isSelf = u.id === currentUserId;
        return (
          <div key={u.id} className={styles.userCard}>
            <div className={styles.userCardHeader}>
              <div className={styles.userCardEmail}>
                {u.email}
                {u.username && <span className={styles.username}>{u.username}</span>}
              </div>
              {statusBadge(u.status)}
            </div>
            <div className={styles.userCardMeta}>
              <span>{u.auth_provider}</span>
              <span>Joined {new Date(u.created_at).toLocaleDateString()}</span>
            </div>
            <div className={styles.userCardFooter}>
              <select
                value={u.role}
                disabled={isSelf}
                onChange={(e) => roleMutation.mutate({ userId: u.id, role: e.target.value })}
                className={styles.roleSelect}
              >
                <option value="admin">admin</option>
                <option value="user">user</option>
              </select>
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
            </div>
          </div>
        );
      })}
    </div>
  );
}

function CreateInviteSuccess({
  invite,
  emailSentTo,
  onDismiss,
}: {
  invite: InviteCode;
  emailSentTo: string | null;
  onDismiss: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const [showCode, setShowCode] = useState(false);
  const link = inviteLink(invite.code);

  const copyLink = () => {
    navigator.clipboard.writeText(link);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className={styles.successCard} data-testid="invite-success">
      <div className={styles.successHeader}>
        <strong>Invite created</strong>
        <button type="button" className="op-btn op-btn-ghost op-btn-sm" onClick={onDismiss}>
          Dismiss
        </button>
      </div>
      {emailSentTo && <p className={styles.emailSent}>Email sent to {emailSentTo}</p>}
      <div className={styles.linkRow}>
        <code className={styles.linkCode}>{link}</code>
        <button type="button" className="op-btn op-btn-primary op-btn-sm" onClick={copyLink}>
          {copied ? "Copied!" : "Copy Link"}
        </button>
      </div>
      <button
        type="button"
        className={`op-btn op-btn-ghost op-btn-sm ${styles.showCodeToggle}`}
        onClick={() => setShowCode(!showCode)}
      >
        {showCode ? "Hide code" : "Show code"}
      </button>
      {showCode && <code className={styles.rawCode}>{invite.code}</code>}
    </div>
  );
}

function InviteCard({ invite }: { invite: InviteCode }) {
  const queryClient = useQueryClient();
  const [copied, setCopied] = useState(false);
  const [showCode, setShowCode] = useState(false);
  const [showEmailForm, setShowEmailForm] = useState(false);
  const [email, setEmail] = useState("");
  const [emailSent, setEmailSent] = useState(false);

  const status = inviteStatusLabel(invite);
  const link = inviteLink(invite.code);

  const revokeMutation = useMutation({
    mutationFn: (id: string) => adminApi.revokeInvite(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["admin-invites"] }),
  });

  const sendEmailMutation = useMutation({
    mutationFn: ({ id, emailAddr }: { id: string; emailAddr: string }) =>
      adminApi.sendInviteEmail(id, emailAddr),
    onSuccess: () => {
      setEmailSent(true);
      setShowEmailForm(false);
      setEmail("");
      setTimeout(() => setEmailSent(false), 3000);
    },
  });

  const copyLink = () => {
    navigator.clipboard.writeText(link);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleRevoke = () => {
    if (window.confirm("Are you sure? This invite link will stop working.")) {
      revokeMutation.mutate(invite.id);
    }
  };

  const handleSendEmail = (e: React.FormEvent) => {
    e.preventDefault();
    if (email.trim()) {
      sendEmailMutation.mutate({ id: invite.id, emailAddr: email.trim() });
    }
  };

  const usesPercent =
    invite.max_uses != null ? Math.min(100, (invite.use_count / invite.max_uses) * 100) : null;

  return (
    <div className={styles.inviteCard} data-testid="invite-card">
      <div className={styles.cardHeader}>
        <span className={styles.cardLabel}>{invite.label || "Untitled invite"}</span>
        <span className={inviteStatusBadgeClass(status)}>{status}</span>
      </div>

      <div className={styles.linkRow}>
        <code className={styles.linkCode}>{link}</code>
      </div>
      <div className={styles.cardActions}>
        <button type="button" className="op-btn op-btn-primary op-btn-sm" onClick={copyLink}>
          {copied ? "Copied!" : "Copy Link"}
        </button>
        <button
          type="button"
          className="op-btn op-btn-ghost op-btn-sm"
          onClick={() => setShowCode(!showCode)}
        >
          {showCode ? "Hide Code" : "Show Code"}
        </button>
      </div>
      {showCode && <code className={styles.rawCode}>{invite.code}</code>}

      <div className={styles.cardMeta}>
        <span>
          Uses: {invite.use_count}/{invite.max_uses ?? "\u221E"}
        </span>
        {invite.expires_at && (
          <span>Expires: {new Date(invite.expires_at).toLocaleDateString()}</span>
        )}
      </div>
      {usesPercent != null && (
        <div
          className={styles.progressBar}
          role="progressbar"
          aria-valuenow={invite.use_count}
          aria-valuemin={0}
          aria-valuemax={invite.max_uses ?? 0}
        >
          <div className={styles.progressFill} style={{ width: `${usesPercent}%` }} />
        </div>
      )}

      <div className={styles.cardFooter}>
        {status === "Active" && (
          <>
            <button
              type="button"
              className="op-btn op-btn-ghost op-btn-sm"
              onClick={() => setShowEmailForm(!showEmailForm)}
            >
              {emailSent ? "Sent!" : "Send Email"}
            </button>
            <button type="button" className="op-btn op-btn-danger op-btn-sm" onClick={handleRevoke}>
              Revoke
            </button>
          </>
        )}
      </div>

      {showEmailForm && (
        <form onSubmit={handleSendEmail} className={styles.emailForm}>
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="recipient@example.com"
            className={styles.createFormInput}
            required
            aria-label="Recipient email"
          />
          <button
            type="submit"
            className="op-btn op-btn-primary op-btn-sm"
            disabled={sendEmailMutation.isPending}
          >
            Send
          </button>
        </form>
      )}
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
  const [sendToEmail, setSendToEmail] = useState("");
  const [createdInvite, setCreatedInvite] = useState<InviteCode | null>(null);
  const [createdEmailSentTo, setCreatedEmailSentTo] = useState<string | null>(null);

  const createMutation = useMutation({
    mutationFn: (data: CreateInviteRequest) => adminApi.createInvite(data),
    onSuccess: (invite) => {
      queryClient.invalidateQueries({ queryKey: ["admin-invites"] });
      setCreatedInvite(invite);
      setCreatedEmailSentTo(sendToEmail.trim() || null);
      setShowForm(false);
      setLabel("");
      setMaxUses("");
      setExpiresInHours("");
      setSendToEmail("");
    },
  });

  const handleCreate = (e: React.FormEvent) => {
    e.preventDefault();
    createMutation.mutate({
      label: label || undefined,
      max_uses: maxUses ? Number.parseInt(maxUses, 10) : undefined,
      expires_in_hours: expiresInHours ? Number.parseInt(expiresInHours, 10) : undefined,
      send_to_email: sendToEmail.trim() || undefined,
    });
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
          <div className={styles.createFormField}>
            <label htmlFor="invite-email" className={styles.createFormLabel}>
              Send to email (optional)
            </label>
            <input
              id="invite-email"
              type="email"
              value={sendToEmail}
              onChange={(e) => setSendToEmail(e.target.value)}
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
        <CreateInviteSuccess
          invite={createdInvite}
          emailSentTo={createdEmailSentTo}
          onDismiss={() => setCreatedInvite(null)}
        />
      )}

      <div className={styles.inviteGrid}>
        {isLoading ? (
          <p className={styles.loadingText}>Loading invites...</p>
        ) : invites && invites.length > 0 ? (
          invites.map((inv: InviteCode) => <InviteCard key={inv.id} invite={inv} />)
        ) : (
          <p className={styles.emptyText}>No invites yet.</p>
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
