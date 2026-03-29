// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface InviteCheckResponse {
  valid: boolean;
  label: string | null;
  expires_at: string | null;
  inviter_name: string | null;
  reason?: "expired" | "revoked" | "exhausted" | "not_found";
}

export interface InviteClaim {
  user_email: string;
  claimed_at: string;
}

export interface SendInviteEmailRequest {
  email: string;
}

export const invitesApi = {
  check: (code: string) => api.get<InviteCheckResponse>(`/api/v1/invites/${code}/check`),
  getClaims: (inviteId: string) =>
    api.get<InviteClaim[]>(`/api/v1/admin/invites/${inviteId}/claims`),
  sendEmail: (inviteId: string, data: SendInviteEmailRequest) =>
    api.post<void>(`/api/v1/admin/invites/${inviteId}/send-email`, data),
};
