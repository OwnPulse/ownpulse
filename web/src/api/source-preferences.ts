// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface SourcePreference {
  id: string;
  metric_type: string;
  preferred_source: string;
}

export interface UpsertSourcePreference {
  metric_type: string;
  preferred_source: string;
}

export const sourcePreferencesApi = {
  list: () => api.get<SourcePreference[]>("/api/v1/source-preferences"),
  upsert: (data: UpsertSourcePreference) =>
    api.put<SourcePreference>("/api/v1/source-preferences", data),
};
