// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { accountApi } from "../api/account";
import { getAuthMethods, logout, unlinkAuth } from "../api/auth";
import { exportCsv, exportJson } from "../api/export";
import { sourcePreferencesApi } from "../api/source-preferences";
import NotificationSettings from "../components/settings/NotificationSettings";
import { useTheme } from "../hooks/useTheme";
import styles from "./Settings.module.css";

const THEME_OPTIONS = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "system", label: "System" },
] as const;

const LINKABLE_PROVIDERS = ["google"] as const;

const SETTINGS_MESSAGES: Record<string, { type: "success" | "error"; text: string }> = {
  "linked=google": { type: "success", text: "Google account linked successfully." },
  "error=already_linked": {
    type: "error",
    text: "That Google account is already linked to a different user.",
  },
  "error=auth_required": { type: "error", text: "Your session expired. Please log in again." },
};

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
  const [searchParams, setSearchParams] = useSearchParams();
  const [statusMsg, setStatusMsg] = useState<{ type: "success" | "error"; text: string } | null>(
    null,
  );

  useEffect(() => {
    const linked = searchParams.get("linked");
    const error = searchParams.get("error");

    let key: string | null = null;
    if (linked) key = `linked=${linked}`;
    else if (error) key = `error=${error}`;

    if (key && key in SETTINGS_MESSAGES) {
      setStatusMsg(SETTINGS_MESSAGES[key]);
      setSearchParams({}, { replace: true });
    }
  }, [searchParams, setSearchParams]);

  const {
    data: methods,
    isLoading,
    isError,
  } = useQuery({
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
    if (
      !window.confirm(
        `Unlink ${providerDisplayName(provider)}? You won't be able to log in with it anymore.`,
      )
    ) {
      return;
    }
    unlinkMutation.mutate(provider);
  };

  const availableToLink = methods
    ? LINKABLE_PROVIDERS.filter((p) => !methods.some((m) => m.provider === p))
    : [];

  return (
    <section className="op-section">
      <h2>Linked Accounts</h2>
      {statusMsg && (
        <p className={statusMsg.type === "error" ? "op-error-msg" : "op-success-msg"}>
          {statusMsg.text}
        </p>
      )}
      {isLoading && <p>Loading...</p>}
      {isError && <p>Error loading linked accounts.</p>}
      {unlinkError && <p className="op-error-msg">{unlinkError}</p>}
      {methods && methods.length === 0 && <p>No linked accounts.</p>}
      {methods && methods.length > 0 && (
        <ul className={styles.linkedList}>
          {methods.map((method) => (
            <li key={method.id} className={styles.linkedAccount}>
              <strong>{providerDisplayName(method.provider)}</strong>
              {method.email && <span className={styles.linkedAccountEmail}>{method.email}</span>}
              {methods.length > 1 && (
                <button
                  type="button"
                  className="op-btn op-btn-ghost op-btn-sm"
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
      {availableToLink.length > 0 && (
        <div className={styles.linkActions}>
          {availableToLink.map((provider) => (
            <a
              key={provider}
              href={`/api/v1/auth/${provider}/login?mode=link`}
              className="op-btn op-btn-secondary op-btn-sm"
            >
              Link {providerDisplayName(provider)}
            </a>
          ))}
        </div>
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

  const { theme, setTheme } = useTheme();

  return (
    <main className="op-page">
      <h1>Settings</h1>

      <section className="op-section">
        <h2>Appearance</h2>
        <fieldset className={styles.themePicker} aria-label="Theme">
          {THEME_OPTIONS.map((option) => (
            <label
              key={option.value}
              className={`${styles.themeOption}${theme === option.value ? ` ${styles.themeOptionActive}` : ""}`}
            >
              <input
                type="radio"
                name="theme"
                value={option.value}
                checked={theme === option.value}
                onChange={() => setTheme(option.value)}
                className={styles.themeRadioInput}
              />
              {option.label}
            </label>
          ))}
        </fieldset>
      </section>

      <section>
        <h2>Export Data</h2>
        <p>Download all your data in open formats.</p>
        <div className={styles.exportButtons}>
          <button
            type="button"
            className="op-btn op-btn-secondary"
            onClick={handleExportJson}
            disabled={exporting}
          >
            {exporting ? "Exporting..." : "Export JSON"}
          </button>
          <button
            type="button"
            className="op-btn op-btn-secondary"
            onClick={handleExportCsv}
            disabled={exporting}
          >
            {exporting ? "Exporting..." : "Export CSV"}
          </button>
        </div>
      </section>

      <section className="op-section">
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

      <NotificationSettings />

      <div className={styles.dangerZone}>
        <h2 className={styles.dangerTitle}>Danger Zone</h2>
        {!confirmDelete ? (
          <button type="button" className="op-btn op-btn-danger" onClick={handleDeleteAccount}>
            Delete Account
          </button>
        ) : (
          <div>
            <p>Are you sure? This will permanently delete your account and all data.</p>
            <div className={styles.confirmActions}>
              <button type="button" className="op-btn op-btn-danger" onClick={handleDeleteAccount}>
                Yes, delete my account
              </button>
              <button
                type="button"
                className="op-btn op-btn-ghost"
                onClick={() => setConfirmDelete(false)}
              >
                Cancel
              </button>
            </div>
          </div>
        )}
      </div>
    </main>
  );
}
