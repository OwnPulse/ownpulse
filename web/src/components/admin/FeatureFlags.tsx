// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { adminApi, type FeatureFlag } from "../../api/admin";
import styles from "./FeatureFlags.module.css";

export function FeatureFlagsSection() {
  const queryClient = useQueryClient();
  const [showForm, setShowForm] = useState(false);
  const [newKey, setNewKey] = useState("");
  const [newDescription, setNewDescription] = useState("");
  const [newEnabled, setNewEnabled] = useState(false);

  const {
    data: flags,
    isLoading,
    isError,
  } = useQuery({
    queryKey: ["admin-feature-flags"],
    queryFn: adminApi.listFeatureFlags,
  });

  const toggleMutation = useMutation({
    mutationFn: ({ key, enabled }: { key: string; enabled: boolean }) =>
      adminApi.upsertFeatureFlag(key, { enabled }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["admin-feature-flags"] }),
  });

  const createMutation = useMutation({
    mutationFn: ({
      key,
      enabled,
      description,
    }: {
      key: string;
      enabled: boolean;
      description?: string;
    }) => adminApi.upsertFeatureFlag(key, { enabled, description }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["admin-feature-flags"] });
      setShowForm(false);
      setNewKey("");
      setNewDescription("");
      setNewEnabled(false);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (key: string) => adminApi.deleteFeatureFlag(key),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["admin-feature-flags"] }),
  });

  const handleCreate = (e: React.FormEvent) => {
    e.preventDefault();
    const trimmedKey = newKey.trim();
    if (!trimmedKey) return;
    createMutation.mutate({
      key: trimmedKey,
      enabled: newEnabled,
      description: newDescription.trim() || undefined,
    });
  };

  const handleDelete = (flag: FeatureFlag) => {
    if (window.confirm(`Delete flag "${flag.key}"? This cannot be undone.`)) {
      deleteMutation.mutate(flag.key);
    }
  };

  return (
    <div>
      <div className={styles.header}>
        <h2>Feature Flags</h2>
        <button
          type="button"
          className="op-btn op-btn-primary op-btn-sm"
          onClick={() => setShowForm(!showForm)}
        >
          {showForm ? "Cancel" : "New Flag"}
        </button>
      </div>

      {showForm && (
        <form onSubmit={handleCreate} className={styles.createForm} data-testid="create-flag-form">
          <div className={styles.createFormField}>
            <label htmlFor="flag-key" className={styles.createFormLabel}>
              Key
            </label>
            <input
              id="flag-key"
              type="text"
              value={newKey}
              onChange={(e) => setNewKey(e.target.value)}
              placeholder="e.g. dark_mode_v2"
              className={styles.createFormInput}
              required
            />
          </div>
          <div className={styles.createFormField}>
            <label htmlFor="flag-description" className={styles.createFormLabel}>
              Description
            </label>
            <input
              id="flag-description"
              type="text"
              value={newDescription}
              onChange={(e) => setNewDescription(e.target.value)}
              placeholder="Optional description"
              className={styles.createFormInput}
            />
          </div>
          <div className={styles.createFormField}>
            <span className={styles.createFormLabel}>Enabled</span>
            <label className={styles.toggle}>
              <input
                type="checkbox"
                checked={newEnabled}
                onChange={(e) => setNewEnabled(e.target.checked)}
                data-testid="new-flag-enabled-toggle"
              />
              <span className={styles.toggleTrack} />
            </label>
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

      {isLoading && <p>Loading feature flags...</p>}
      {isError && <p>Error loading feature flags.</p>}

      {flags && flags.length > 0 ? (
        <div className={styles.flagList} data-testid="flag-list">
          {flags.map((flag: FeatureFlag) => (
            <div key={flag.id} className={styles.flagRow} data-testid={`flag-row-${flag.key}`}>
              <div className={styles.flagInfo}>
                <span className={styles.flagKey}>{flag.key}</span>
                {flag.description && (
                  <span className={styles.flagDescription}>{flag.description}</span>
                )}
              </div>
              <div className={styles.flagActions}>
                <label className={styles.toggle}>
                  <input
                    type="checkbox"
                    checked={flag.enabled}
                    onChange={() =>
                      toggleMutation.mutate({
                        key: flag.key,
                        enabled: !flag.enabled,
                      })
                    }
                    aria-label={`Toggle ${flag.key}`}
                    data-testid={`toggle-${flag.key}`}
                  />
                  <span className={styles.toggleTrack} />
                </label>
                <button
                  type="button"
                  className="op-btn op-btn-ghost op-btn-sm"
                  onClick={() => handleDelete(flag)}
                >
                  Delete
                </button>
              </div>
            </div>
          ))}
        </div>
      ) : (
        !isLoading && !isError && <p className={styles.emptyText}>No feature flags yet.</p>
      )}
    </div>
  );
}
