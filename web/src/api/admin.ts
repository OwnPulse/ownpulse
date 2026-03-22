// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

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

export const adminApi = {
  listUsers: () => api.get<AdminUser[]>("/api/v1/admin/users"),
  updateRole: (userId: string, role: string) =>
    api.patch<AdminUser>(`/api/v1/admin/users/${userId}/role`, { role }),
  updateUserStatus: (userId: string, status: string) =>
    api.patch<AdminUser>(`/api/v1/admin/users/${userId}/status`, { status }),
  deleteUser: (userId: string) => api.delete<void>(`/api/v1/admin/users/${userId}`),
  listInvites: () => api.get<InviteCode[]>("/api/v1/admin/invites"),
  createInvite: (data: { label?: string; max_uses?: number; expires_in_hours?: number }) =>
    api.post<InviteCode>("/api/v1/admin/invites", data),
  revokeInvite: (id: string) => api.delete<InviteCode>(`/api/v1/admin/invites/${id}`),
};
