// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface IntegrationStatus {
  source: string;
  connected: boolean;
  last_synced_at?: string;
  last_sync_error?: string;
}

export const integrationsApi = {
  list: () => api.get<IntegrationStatus[]>("/api/v1/integrations"),
  disconnect: (source: string) => api.delete<void>(`/api/v1/integrations/${source}`),
};
