// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface UserAccount {
  id: string;
  username: string;
  auth_provider: string;
  email?: string;
  role: string;
  data_region: string;
  created_at: string;
}

export const accountApi = {
  get: () => api.get<UserAccount>("/api/v1/account"),
  delete: () => api.delete<void>("/api/v1/account"),
};
