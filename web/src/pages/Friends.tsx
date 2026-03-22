// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link } from "react-router-dom";
import { DATA_TYPES, type FriendShare, friendsApi } from "../api/friends";
import styles from "./Friends.module.css";

function DataTypePills({ types }: { types: string[] }) {
  return (
    <span>
      {types.map((t) => {
        const label = DATA_TYPES.find((dt) => dt.value === t)?.label ?? t;
        return (
          <span key={t} className="op-pill">
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
    <main className={`op-page ${styles.page}`}>
      <h1>Friends</h1>

      {/* Share Data form */}
      <section className={`op-card ${styles.shareForm}`}>
        <h2>Share Data</h2>
        <div className="op-form-field">
          <label htmlFor="friend-email" className="op-label">
            Friend&apos;s email (leave blank for invite link)
          </label>
          <input
            id="friend-email"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="friend@example.com"
            className={`op-input ${styles.emailInput}`}
          />
        </div>

        <div className="op-form-field">
          <span className="op-label">Data types to share</span>
          <div className={styles.checkboxGroup}>
            {DATA_TYPES.map((dt) => (
              <label key={dt.value} className={styles.checkboxLabel}>
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
          className="op-btn op-btn-primary"
          onClick={() => createShare.mutate()}
          disabled={createShare.isPending || selectedTypes.length === 0}
        >
          {createShare.isPending ? "Sharing..." : "Share"}
        </button>

        {error && <p className={`op-error-msg ${styles.errorMsg}`}>{error}</p>}

        {inviteLink && (
          <div className={styles.inviteLinkBox}>
            <span className={styles.inviteLinkLabel}>Invite link (share with your friend):</span>
            <div className={styles.inviteLinkRow}>
              <input type="text" readOnly value={inviteLink} className={styles.inviteLinkInput} />
              <button
                type="button"
                className="op-btn op-btn-ghost op-btn-sm"
                onClick={() => copyToClipboard(inviteLink)}
              >
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
          <p className={styles.emptyText}>Not sharing with anyone yet.</p>
        )}
        {outgoing.data?.map((share) => (
          <div key={share.id} className={`op-card ${styles.shareCardItem}`}>
            <div className={styles.shareCard}>
              <div>
                <strong>{share.friend_email ?? "Invite link: pending"}</strong>
                <div className={styles.pills}>
                  <DataTypePills types={share.data_types} />
                </div>
              </div>
              <div className={styles.shareActions}>
                {share.invite_token && !share.friend_id && (
                  <button
                    type="button"
                    className="op-btn op-btn-ghost op-btn-sm"
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
                  className="op-btn op-btn-danger op-btn-sm"
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
          <p className={styles.emptyText}>No one is sharing with you yet.</p>
        )}
        {incoming.data?.map((share) => (
          <div key={share.id} className={`op-card ${styles.shareCardItem}`}>
            <div className={styles.shareCard}>
              <div>
                <strong>{share.owner_email}</strong>
                <div className={styles.pills}>
                  <DataTypePills types={share.data_types} />
                </div>
              </div>
              <div className={styles.shareActions}>
                {share.status === "pending" ? (
                  <>
                    <button
                      type="button"
                      className="op-btn op-btn-primary op-btn-sm"
                      onClick={() => acceptShare.mutate(share.id)}
                      disabled={acceptShare.isPending}
                    >
                      Accept
                    </button>
                    <button
                      type="button"
                      className="op-btn op-btn-danger op-btn-sm"
                      onClick={() => revokeShare.mutate(share.id)}
                      disabled={revokeShare.isPending}
                    >
                      Decline
                    </button>
                  </>
                ) : (
                  <Link
                    to={`/friends/${share.owner_id}`}
                    className="op-btn op-btn-primary op-btn-sm"
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
