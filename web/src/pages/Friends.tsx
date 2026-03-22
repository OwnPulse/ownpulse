// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link } from "react-router-dom";
import { DATA_TYPES, type FriendShare, friendsApi } from "../api/friends";

const cardStyle: React.CSSProperties = {
  background: "var(--color-surface)",
  borderRadius: "var(--radius-md)",
  border: "1px solid var(--color-border)",
  padding: "1.25rem",
  marginBottom: "1rem",
};

const pillStyle: React.CSSProperties = {
  display: "inline-block",
  padding: "0.125rem 0.5rem",
  borderRadius: "999px",
  background: "var(--color-primary-light)",
  color: "var(--color-primary)",
  fontSize: "0.75rem",
  fontWeight: 500,
  marginRight: "0.375rem",
};

const btnStyle: React.CSSProperties = {
  padding: "0.375rem 0.75rem",
  borderRadius: "var(--radius-sm)",
  border: "1px solid var(--color-border)",
  background: "var(--color-surface)",
  cursor: "pointer",
  fontSize: "0.8125rem",
  fontFamily: "var(--font-family)",
};

const btnPrimary: React.CSSProperties = {
  ...btnStyle,
  background: "var(--color-primary)",
  color: "#fff",
  border: "none",
};

function DataTypePills({ types }: { types: string[] }) {
  return (
    <span>
      {types.map((t) => {
        const label = DATA_TYPES.find((dt) => dt.value === t)?.label ?? t;
        return (
          <span key={t} style={pillStyle}>
            {label}
          </span>
        );
      })}
    </span>
  );
}

export default function Friends() {
  const queryClient = useQueryClient();
  const [email, setEmail] = useState("");
  const [selectedTypes, setSelectedTypes] = useState<string[]>(["checkins"]);
  const [inviteLink, setInviteLink] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const outgoing = useQuery({
    queryKey: ["friends", "outgoing"],
    queryFn: () => friendsApi.listOutgoing(),
  });

  const incoming = useQuery({
    queryKey: ["friends", "incoming"],
    queryFn: () => friendsApi.listIncoming(),
  });

  const createShare = useMutation({
    mutationFn: () => friendsApi.createShare(email.trim() || null, selectedTypes),
    onSuccess: (share: FriendShare) => {
      setError(null);
      if (share.invite_token) {
        const link = `${window.location.origin}/share/accept?token=${share.invite_token}`;
        setInviteLink(link);
      } else {
        setInviteLink(null);
        setEmail("");
      }
      queryClient.invalidateQueries({ queryKey: ["friends"] });
    },
    onError: (err: Error) => {
      setError(err.message);
      setInviteLink(null);
    },
  });

  const acceptShare = useMutation({
    mutationFn: (shareId: string) => friendsApi.acceptShare(shareId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["friends"] });
    },
  });

  const revokeShare = useMutation({
    mutationFn: (shareId: string) => friendsApi.revokeShare(shareId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["friends"] });
    },
  });

  const toggleType = (value: string) => {
    setSelectedTypes((prev) =>
      prev.includes(value) ? prev.filter((v) => v !== value) : [...prev, value],
    );
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
  };

  return (
    <main style={{ padding: "1.5rem", maxWidth: "48rem", margin: "0 auto" }}>
      <h1>Friends</h1>

      {/* Share Data form */}
      <section style={cardStyle}>
        <h2 style={{ marginTop: 0 }}>Share Data</h2>
        <div style={{ marginBottom: "0.75rem" }}>
          <label
            htmlFor="friend-email"
            style={{ display: "block", marginBottom: "0.25rem", fontSize: "0.875rem" }}
          >
            Friend&apos;s email (leave blank for invite link)
          </label>
          <input
            id="friend-email"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="friend@example.com"
            style={{
              padding: "0.375rem 0.5rem",
              borderRadius: "var(--radius-sm)",
              border: "1px solid var(--color-border)",
              fontFamily: "var(--font-family)",
              fontSize: "0.875rem",
              width: "100%",
              maxWidth: "20rem",
              boxSizing: "border-box",
            }}
          />
        </div>

        <div style={{ marginBottom: "0.75rem" }}>
          <span style={{ display: "block", marginBottom: "0.25rem", fontSize: "0.875rem" }}>
            Data types to share
          </span>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "0.5rem" }}>
            {DATA_TYPES.map((dt) => (
              <label
                key={dt.value}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "0.25rem",
                  fontSize: "0.8125rem",
                }}
              >
                <input
                  type="checkbox"
                  checked={selectedTypes.includes(dt.value)}
                  onChange={() => toggleType(dt.value)}
                />
                {dt.label}
              </label>
            ))}
          </div>
        </div>

        <button
          type="button"
          style={btnPrimary}
          onClick={() => createShare.mutate()}
          disabled={createShare.isPending || selectedTypes.length === 0}
        >
          {createShare.isPending ? "Sharing..." : "Share"}
        </button>

        {error && (
          <p
            style={{ color: "var(--color-error, red)", marginTop: "0.5rem", fontSize: "0.875rem" }}
          >
            {error}
          </p>
        )}

        {inviteLink && (
          <div
            style={{
              marginTop: "0.75rem",
              padding: "0.75rem",
              background: "var(--color-background)",
              borderRadius: "var(--radius-sm)",
              border: "1px solid var(--color-border)",
            }}
          >
            <span style={{ display: "block", fontSize: "0.8125rem", marginBottom: "0.25rem" }}>
              Invite link (share with your friend):
            </span>
            <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
              <input
                type="text"
                readOnly
                value={inviteLink}
                style={{
                  flex: 1,
                  padding: "0.375rem 0.5rem",
                  borderRadius: "var(--radius-sm)",
                  border: "1px solid var(--color-border)",
                  fontFamily: "var(--font-family)",
                  fontSize: "0.8125rem",
                }}
              />
              <button type="button" style={btnStyle} onClick={() => copyToClipboard(inviteLink)}>
                Copy
              </button>
            </div>
          </div>
        )}
      </section>

      {/* Sharing with (outgoing) */}
      <section>
        <h2>Sharing with</h2>
        {outgoing.isLoading && <p>Loading...</p>}
        {outgoing.isError && <p>Error loading shares.</p>}
        {outgoing.data && outgoing.data.length === 0 && (
          <p style={{ color: "var(--color-text-muted)", fontSize: "0.875rem" }}>
            Not sharing with anyone yet.
          </p>
        )}
        {outgoing.data?.map((share) => (
          <div key={share.id} style={cardStyle}>
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                flexWrap: "wrap",
                gap: "0.5rem",
              }}
            >
              <div>
                <strong>{share.friend_email ?? "Invite link: pending"}</strong>
                <div style={{ marginTop: "0.25rem" }}>
                  <DataTypePills types={share.data_types} />
                </div>
              </div>
              <div style={{ display: "flex", gap: "0.375rem" }}>
                {share.invite_token && !share.friend_id && (
                  <button
                    type="button"
                    style={btnStyle}
                    onClick={() =>
                      copyToClipboard(
                        `${window.location.origin}/share/accept?token=${share.invite_token}`,
                      )
                    }
                  >
                    Copy link
                  </button>
                )}
                <button
                  type="button"
                  style={{ ...btnStyle, color: "var(--color-error, red)" }}
                  onClick={() => revokeShare.mutate(share.id)}
                  disabled={revokeShare.isPending}
                >
                  Revoke
                </button>
              </div>
            </div>
          </div>
        ))}
      </section>

      {/* Shared with me (incoming) */}
      <section>
        <h2>Shared with me</h2>
        {incoming.isLoading && <p>Loading...</p>}
        {incoming.isError && <p>Error loading shares.</p>}
        {incoming.data && incoming.data.length === 0 && (
          <p style={{ color: "var(--color-text-muted)", fontSize: "0.875rem" }}>
            No one is sharing with you yet.
          </p>
        )}
        {incoming.data?.map((share) => (
          <div key={share.id} style={cardStyle}>
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                flexWrap: "wrap",
                gap: "0.5rem",
              }}
            >
              <div>
                <strong>{share.owner_email}</strong>
                <div style={{ marginTop: "0.25rem" }}>
                  <DataTypePills types={share.data_types} />
                </div>
              </div>
              <div style={{ display: "flex", gap: "0.375rem" }}>
                {share.status === "pending" ? (
                  <>
                    <button
                      type="button"
                      style={btnPrimary}
                      onClick={() => acceptShare.mutate(share.id)}
                      disabled={acceptShare.isPending}
                    >
                      Accept
                    </button>
                    <button
                      type="button"
                      style={{ ...btnStyle, color: "var(--color-error, red)" }}
                      onClick={() => revokeShare.mutate(share.id)}
                      disabled={revokeShare.isPending}
                    >
                      Decline
                    </button>
                  </>
                ) : (
                  <Link
                    to={`/friends/${share.owner_id}`}
                    style={{
                      ...btnPrimary,
                      textDecoration: "none",
                      display: "inline-block",
                    }}
                  >
                    View
                  </Link>
                )}
              </div>
            </div>
          </div>
        ))}
      </section>
    </main>
  );
}
