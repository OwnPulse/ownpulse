// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";
import type { ProtocolExport } from "./protocols";

export interface FeatureFlag {
  id: string;
  key: string;
  enabled: boolean;
  description: string | null;
  created_at: string;
  updated_at: string;
}

export interface UpsertFlagRequest {
  enabled: boolean;
  description?: string;
}

export interface AdminUser {
  id: string;
  username?: string;
  auth_provider: string;
  email: string;
  role: string;
  status: string;
  data_region: string;
  created_at: string;
}

export interface InviteCode {
  id: string;
  code: string;
  label?: string;
  max_uses?: number;
  use_count: number;
  expires_at?: string;
  revoked_at?: string;
  created_at: string;
}

export interface CreateInviteRequest {
  label?: string;
  max_uses?: number;
  expires_in_hours?: number;
  send_to_email?: string;
}

export const adminApi = {
  listUsers: () => api.get<AdminUser[]>("/api/v1/admin/users"),
  updateRole: (userId: string, role: string) =>
    api.patch<AdminUser>(`/api/v1/admin/users/${userId}/role`, { role }),
  updateUserStatus: (userId: string, status: string) =>
    api.patch<AdminUser>(`/api/v1/admin/users/${userId}/status`, { status }),
  deleteUser: (userId: string) => api.delete<void>(`/api/v1/admin/users/${userId}`),
  listInvites: () => api.get<InviteCode[]>("/api/v1/admin/invites"),
  createInvite: (data: CreateInviteRequest) => api.post<InviteCode>("/api/v1/admin/invites", data),
  revokeInvite: (id: string) => api.delete<InviteCode>(`/api/v1/admin/invites/${id}`),
  sendInviteEmail: (id: string, email: string) =>
    api.post<void>(`/api/v1/admin/invites/${id}/send-email`, { email }),
  promoteProtocol: (id: string, tags: string[]) =>
    api.post<void>(`/api/v1/admin/protocols/${id}/promote`, { tags }),
  demoteProtocol: (id: string) => api.post<void>(`/api/v1/admin/protocols/${id}/demote`, {}),
  bulkImportProtocols: (data: { url?: string; protocols?: ProtocolExport[] }) =>
    api.post<{ imported: number }>("/api/v1/admin/protocols/import", data),
  listFeatureFlags: () => api.get<FeatureFlag[]>("/api/v1/admin/feature-flags"),
  upsertFeatureFlag: (key: string, data: UpsertFlagRequest) =>
    api.put<FeatureFlag>(
      `/api/v1/admin/feature-flags/${encodeURIComponent(key)}`,
      data,
    ),
  deleteFeatureFlag: (key: string) =>
    api.delete<void>(
      `/api/v1/admin/feature-flags/${encodeURIComponent(key)}`,
    ),
};
