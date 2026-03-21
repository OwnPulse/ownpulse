// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface AdminUser {
  id: string;
  username: string;
  auth_provider: string;
  email?: string;
  role: string;
  data_region: string;
  created_at: string;
}

export const adminApi = {
  listUsers: () => api.get<AdminUser[]>("/api/v1/admin/users"),
  updateRole: (userId: string, role: string) =>
    api.patch<AdminUser>(`/api/v1/admin/users/${userId}/role`, { role }),
};
