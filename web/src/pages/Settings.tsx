// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { exportJson, exportCsv } from "../api/export";
import { sourcePreferencesApi } from "../api/source-preferences";
import { accountApi } from "../api/account";
import { logout, getAuthMethods, unlinkAuth } from "../api/auth";

const PROVIDER_NAMES: Record<string, string> = {
  google: "Google",
  apple: "Apple",
  local: "Password",
};

function providerDisplayName(provider: string): string {
  return PROVIDER_NAMES[provider] ?? provider;
}

function LinkedAccounts() {
  const queryClient = useQueryClient();
  const [unlinkError, setUnlinkError] = useState<string | null>(null);

  const { data: methods, isLoading, isError } = useQuery({
    queryKey: ["authMethods"],
    queryFn: getAuthMethods,
  });

  const unlinkMutation = useMutation({
    mutationFn: (provider: string) => unlinkAuth(provider),
    onSuccess: (updated) => {
      setUnlinkError(null);
      queryClient.setQueryData(["authMethods"], updated);
    },
    onError: (err: Error) => {
      setUnlinkError(err.message || "Failed to unlink account.");
    },
  });

  const handleUnlink = (provider: string) => {
    if (!window.confirm(`Unlink ${providerDisplayName(provider)}? You won't be able to log in with it anymore.`)) {
      return;
    }
    unlinkMutation.mutate(provider);
  };

  return (
    <section style={{ marginTop: "2rem" }}>
      <h2>Linked Accounts</h2>
      {isLoading && <p>Loading...</p>}
      {isError && <p>Error loading linked accounts.</p>}
      {unlinkError && <p style={{ color: "var(--color-error, red)" }}>{unlinkError}</p>}
      {methods && methods.length === 0 && <p>No linked accounts.</p>}
      {methods && methods.length > 0 && (
        <ul style={{ listStyle: "none", padding: 0 }}>
          {methods.map((method) => (
            <li
              key={method.id}
              style={{ display: "flex", alignItems: "center", gap: "1rem", marginBottom: "0.5rem" }}
            >
              <strong>{providerDisplayName(method.provider)}</strong>
              {method.email && <span style={{ color: "var(--color-text-muted)" }}>{method.email}</span>}
              {methods.length > 1 && (
                <button
                  onClick={() => handleUnlink(method.provider)}
                  disabled={unlinkMutation.isPending}
                  aria-label={`Unlink ${method.provider}`}
                >
                  Unlink
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

export default function Settings() {
  const [exporting, setExporting] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const sourcePrefs = useQuery({
    queryKey: ["source-preferences"],
    queryFn: () => sourcePreferencesApi.list(),
  });

  const handleExportJson = async () => {
    setExporting(true);
    try {
      await exportJson();
    } finally {
      setExporting(false);
    }
  };

  const handleExportCsv = async () => {
    setExporting(true);
    try {
      await exportCsv();
    } finally {
      setExporting(false);
    }
  };

  const handleDeleteAccount = async () => {
    if (!confirmDelete) {
      setConfirmDelete(true);
      return;
    }
    await accountApi.delete();
    await logout();
    window.location.href = "/login";
  };

  return (
    <main style={{ padding: "1.5rem" }}>
      <h1>Settings</h1>

      <section>
        <h2>Export Data</h2>
        <p>Download all your data in open formats.</p>
        <div style={{ display: "flex", gap: "0.5rem" }}>
          <button type="button" onClick={handleExportJson} disabled={exporting}>
            {exporting ? "Exporting..." : "Export JSON"}
          </button>
          <button type="button" onClick={handleExportCsv} disabled={exporting}>
            {exporting ? "Exporting..." : "Export CSV"}
          </button>
        </div>
      </section>

      <section style={{ marginTop: "2rem" }}>
        <h2>Source Preferences</h2>
        {sourcePrefs.isLoading && <p>Loading...</p>}
        {sourcePrefs.isError && <p>Error loading source preferences.</p>}
        {sourcePrefs.data && sourcePrefs.data.length === 0 && (
          <p>No source preferences configured.</p>
        )}
        {sourcePrefs.data && sourcePrefs.data.length > 0 && (
          <ul>
            {sourcePrefs.data.map((pref) => (
              <li key={pref.id}>
                <strong>{pref.metric_type}</strong>: {pref.preferred_source}
              </li>
            ))}
          </ul>
        )}
      </section>

      <LinkedAccounts />

      <section style={{ marginTop: "2rem" }}>
        <h2>Danger Zone</h2>
        {!confirmDelete ? (
          <button type="button" onClick={handleDeleteAccount} style={{ color: "red" }}>
            Delete Account
          </button>
        ) : (
          <div>
            <p>Are you sure? This will permanently delete your account and all data.</p>
            <button
              type="button"
              onClick={handleDeleteAccount}
              style={{ color: "red", marginRight: "0.5rem" }}
            >
              Yes, delete my account
            </button>
            <button type="button" onClick={() => setConfirmDelete(false)}>
              Cancel
            </button>
          </div>
        )}
      </section>
    </main>
  );
}
