// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface FriendShare {
  id: string;
  owner_id: string;
  owner_email: string;
  friend_id: string | null;
  friend_email: string | null;
  status: string;
  invite_token: string | null;
  data_types: string[];
  created_at: string;
  accepted_at: string | null;
}

export const friendsApi = {
  listOutgoing: () => api.get<FriendShare[]>("/api/v1/friends/shares/outgoing"),
  listIncoming: () => api.get<FriendShare[]>("/api/v1/friends/shares/incoming"),
  createShare: (friendEmail: string | null, dataTypes: string[]) =>
    api.post<FriendShare>("/api/v1/friends/shares", {
      friend_email: friendEmail,
      data_types: dataTypes,
    }),
  acceptShare: (shareId: string) =>
    api.post<void>(`/api/v1/friends/shares/${shareId}/accept`, {}),
  acceptLink: (token: string) =>
    api.post<FriendShare>("/api/v1/friends/shares/accept-link", { token }),
  revokeShare: (shareId: string) =>
    api.delete<void>(`/api/v1/friends/shares/${shareId}`),
  updatePermissions: (shareId: string, dataTypes: string[]) =>
    api.patch<void>(`/api/v1/friends/shares/${shareId}/permissions`, {
      data_types: dataTypes,
    }),
  getFriendData: (friendId: string) =>
    api.get<Record<string, unknown[]>>(`/api/v1/friends/${friendId}/data`),
};

export const DATA_TYPES = [
  { value: "checkins", label: "Check-ins" },
  { value: "health_records", label: "Health Records" },
  { value: "interventions", label: "Interventions" },
  { value: "observations", label: "Observations" },
  { value: "lab_results", label: "Lab Results" },
] as const;
