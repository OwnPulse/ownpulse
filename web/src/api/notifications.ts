// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface NotificationPreferences {
  default_notify: boolean;
  default_notify_times: string[];
  repeat_reminders: boolean;
  repeat_interval_minutes: number;
}

export const notificationsApi = {
  getPreferences: () => api.get<NotificationPreferences>("/api/v1/notifications/preferences"),
  updatePreferences: (data: NotificationPreferences) =>
    api.put<NotificationPreferences>("/api/v1/notifications/preferences", data),
};
