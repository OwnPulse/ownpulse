// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useCallback, useRef, useState } from "react";
import type { CreateRunRequest } from "../../api/protocols";
import { protocolsApi } from "../../api/protocols";
import styles from "./StartRunModal.module.css";

interface StartRunModalProps {
  protocolId: string;
  protocolName: string;
  onClose: () => void;
  onStarted?: () => void;
}

function todayISO(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function StartRunModal({
  protocolId,
  protocolName,
  onClose,
  onStarted,
}: StartRunModalProps) {
  const queryClient = useQueryClient();
  const [startDate, setStartDate] = useState(todayISO);
  const [notify, setNotify] = useState(false);
  const [notifyEntries, setNotifyEntries] = useState<{ key: number; time: string }[]>([
    { key: 0, time: "08:00" },
  ]);
  const entryKeyRef = useRef(1);
  const [repeatReminders, setRepeatReminders] = useState(false);

  const mutation = useMutation({
    mutationFn: (data: CreateRunRequest) => protocolsApi.startRun(protocolId, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["protocols"] });
      queryClient.invalidateQueries({ queryKey: ["protocols", protocolId] });
      queryClient.invalidateQueries({ queryKey: ["protocol-runs"] });
      queryClient.invalidateQueries({ queryKey: ["active-runs"] });
      onStarted?.();
      onClose();
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const data: CreateRunRequest = {
      start_date: startDate,
      notify,
    };
    if (notify) {
      data.notify_times = notifyEntries.map((e) => e.time);
      data.repeat_reminders = repeatReminders;
    }
    mutation.mutate(data);
  };

  const handleOverlayKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    },
    [onClose],
  );

  const updateNotifyTime = (key: number, value: string) => {
    setNotifyEntries((prev) => prev.map((e) => (e.key === key ? { ...e, time: value } : e)));
  };

  const addNotifyTime = () => {
    const key = entryKeyRef.current++;
    setNotifyEntries((prev) => [...prev, { key, time: "20:00" }]);
  };

  const removeNotifyTime = (key: number) => {
    setNotifyEntries((prev) => prev.filter((e) => e.key !== key));
  };

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: modal overlay backdrop
    <div
      className={styles.overlay}
      role="presentation"
      onClick={onClose}
      onKeyDown={handleOverlayKeyDown}
    >
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: stopPropagation prevents overlay dismiss */}
      <div className={`op-card ${styles.card}`} role="dialog" onClick={(e) => e.stopPropagation()}>
        <h2>Start Run</h2>
        <p
          style={{
            fontSize: "var(--text-sm)",
            color: "var(--color-text-muted)",
            margin: "0 0 1rem",
          }}
        >
          {protocolName}
        </p>

        <form onSubmit={handleSubmit}>
          <div className={styles.field}>
            <label className={styles.label} htmlFor="run-start-date">
              Start Date
            </label>
            <input
              id="run-start-date"
              type="date"
              value={startDate}
              onChange={(e) => setStartDate(e.target.value)}
              className={styles.input}
              required
            />
          </div>

          <div className={styles.field}>
            <div className={styles.checkboxRow}>
              <input
                id="run-notify"
                type="checkbox"
                checked={notify}
                onChange={(e) => setNotify(e.target.checked)}
              />
              <label htmlFor="run-notify" className={styles.checkboxLabel}>
                Enable notifications
              </label>
            </div>
          </div>

          {notify && (
            <>
              <div className={styles.field}>
                <span className={styles.label}>Notification Times</span>
                <div className={styles.notifyTimes}>
                  {notifyEntries.map((entry) => (
                    <div
                      key={entry.key}
                      style={{ display: "flex", alignItems: "center", gap: "0.25rem" }}
                    >
                      <input
                        type="time"
                        value={entry.time}
                        onChange={(e) => updateNotifyTime(entry.key, e.target.value)}
                        aria-label={`Notification time`}
                      />
                      {notifyEntries.length > 1 && (
                        <button
                          type="button"
                          className="op-btn op-btn-ghost op-btn-sm"
                          onClick={() => removeNotifyTime(entry.key)}
                          aria-label="Remove time"
                        >
                          &times;
                        </button>
                      )}
                    </div>
                  ))}
                </div>
                <button
                  type="button"
                  className="op-btn op-btn-ghost op-btn-sm"
                  onClick={addNotifyTime}
                  style={{ marginTop: "0.25rem" }}
                >
                  + Add time
                </button>
              </div>

              <div className={styles.field}>
                <div className={styles.checkboxRow}>
                  <input
                    id="run-repeat"
                    type="checkbox"
                    checked={repeatReminders}
                    onChange={(e) => setRepeatReminders(e.target.checked)}
                  />
                  <label htmlFor="run-repeat" className={styles.checkboxLabel}>
                    Repeat if not logged (every 30 min)
                  </label>
                </div>
              </div>
            </>
          )}

          {mutation.isError && (
            <p className="op-error-msg" role="alert">
              Failed to start run: {mutation.error.message}
            </p>
          )}

          <div className={styles.actions}>
            <button type="button" className="op-btn op-btn-ghost" onClick={onClose}>
              Cancel
            </button>
            <button type="submit" className="op-btn op-btn-primary" disabled={mutation.isPending}>
              {mutation.isPending ? "Starting..." : "Start Run"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
