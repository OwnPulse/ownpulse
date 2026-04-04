// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { type NotificationPreferences, notificationsApi } from "../../api/notifications";
import forms from "../forms/forms.module.css";

interface TimeEntry {
  id: number;
  value: string;
}

export default function NotificationSettings() {
  const queryClient = useQueryClient();
  const nextId = useRef(1);
  const [defaultNotify, setDefaultNotify] = useState(false);
  const [notifyTimes, setNotifyTimes] = useState<TimeEntry[]>([{ id: 0, value: "08:00" }]);
  const [repeatReminders, setRepeatReminders] = useState(false);
  const [repeatInterval, setRepeatInterval] = useState(30);

  const prefsQuery = useQuery({
    queryKey: ["notification-preferences"],
    queryFn: () => notificationsApi.getPreferences(),
  });

  useEffect(() => {
    if (prefsQuery.data) {
      setDefaultNotify(prefsQuery.data.default_notify);
      const times =
        prefsQuery.data.default_notify_times.length > 0
          ? prefsQuery.data.default_notify_times
          : ["08:00"];
      const entries = times.map((t) => ({ id: nextId.current++, value: t }));
      setNotifyTimes(entries);
      setRepeatReminders(prefsQuery.data.repeat_reminders);
      setRepeatInterval(prefsQuery.data.repeat_interval_minutes);
    }
  }, [prefsQuery.data]);

  const mutation = useMutation({
    mutationFn: (data: NotificationPreferences) => notificationsApi.updatePreferences(data),
    onSuccess: (updated) => {
      queryClient.setQueryData(["notification-preferences"], updated);
    },
  });

  const handleAddTime = () => {
    setNotifyTimes([...notifyTimes, { id: nextId.current++, value: "12:00" }]);
  };

  const handleRemoveTime = (id: number) => {
    setNotifyTimes(notifyTimes.filter((entry) => entry.id !== id));
  };

  const handleTimeChange = (id: number, value: string) => {
    setNotifyTimes(notifyTimes.map((entry) => (entry.id === id ? { ...entry, value } : entry)));
  };

  const handleSave = () => {
    mutation.mutate({
      default_notify: defaultNotify,
      default_notify_times: notifyTimes.map((entry) => entry.value),
      repeat_reminders: repeatReminders,
      repeat_interval_minutes: repeatInterval,
    });
  };

  if (prefsQuery.isLoading) {
    return (
      <section className="op-section">
        <h2>Protocol Notifications</h2>
        <p>Loading...</p>
      </section>
    );
  }

  if (prefsQuery.isError) {
    return (
      <section className="op-section">
        <h2>Protocol Notifications</h2>
        <p className="op-error-msg">Error loading notification preferences.</p>
      </section>
    );
  }

  return (
    <section className="op-section">
      <h2>Protocol Notifications</h2>
      <p>Configure default notification settings for protocol runs.</p>

      <div className={forms.checkboxField}>
        <input
          type="checkbox"
          id="default-notify"
          checked={defaultNotify}
          onChange={(e) => setDefaultNotify(e.target.checked)}
        />
        <label htmlFor="default-notify" className={forms.checkboxLabel}>
          Enable notifications for new protocol runs
        </label>
      </div>

      {defaultNotify && (
        <>
          <div className={forms.field}>
            <span className={forms.label}>Notification Times</span>
            {notifyTimes.map((entry, index) => (
              <div
                key={entry.id}
                style={{ display: "flex", gap: "0.5rem", marginBottom: "0.5rem" }}
              >
                <input
                  type="time"
                  value={entry.value}
                  onChange={(e) => handleTimeChange(entry.id, e.target.value)}
                  className={forms.input}
                  aria-label={`Notification time ${index + 1}`}
                  style={{ flex: 1 }}
                />
                {notifyTimes.length > 1 && (
                  <button
                    type="button"
                    className="op-btn op-btn-ghost op-btn-sm"
                    onClick={() => handleRemoveTime(entry.id)}
                    aria-label={`Remove time ${index + 1}`}
                  >
                    Remove
                  </button>
                )}
              </div>
            ))}
            <button
              type="button"
              className="op-btn op-btn-secondary op-btn-sm"
              onClick={handleAddTime}
            >
              Add Time
            </button>
          </div>

          <div className={forms.checkboxField}>
            <input
              type="checkbox"
              id="repeat-reminders"
              checked={repeatReminders}
              onChange={(e) => setRepeatReminders(e.target.checked)}
            />
            <label htmlFor="repeat-reminders" className={forms.checkboxLabel}>
              Repeat reminders if dose not logged
            </label>
          </div>

          {repeatReminders && (
            <div className={forms.field}>
              <label className={forms.label} htmlFor="repeat-interval">
                Repeat interval (minutes)
              </label>
              <input
                id="repeat-interval"
                type="number"
                min="5"
                max="120"
                value={repeatInterval}
                onChange={(e) => setRepeatInterval(Number(e.target.value))}
                className={forms.input}
              />
            </div>
          )}
        </>
      )}

      <div className={forms.actions}>
        <button
          type="button"
          className="op-btn op-btn-primary"
          onClick={handleSave}
          disabled={mutation.isPending}
        >
          {mutation.isPending ? "Saving..." : "Save Notification Settings"}
        </button>
      </div>
      {mutation.isError && <p className={forms.errorMsg}>Error: {mutation.error.message}</p>}
      {mutation.isSuccess && <p className={forms.successMsg}>Preferences saved!</p>}
    </section>
  );
}
