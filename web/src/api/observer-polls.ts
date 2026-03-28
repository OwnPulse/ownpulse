// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface PollMemberView {
  id: string;
  observer_email: string;
  accepted_at: string | null;
  created_at: string;
}

export interface Poll {
  id: string;
  name: string;
  custom_prompt: string | null;
  dimensions: string[];
  members: PollMemberView[];
  created_at: string;
  deleted_at: string | null;
}

export interface CreatePollRequest {
  name: string;
  custom_prompt?: string;
  dimensions: string[];
}

export interface UpdatePollRequest {
  name?: string;
  custom_prompt?: string;
}

export interface InviteResponse {
  invite_token: string;
  invite_expires_at: string;
  invite_url: string;
}

export interface AcceptResponse {
  status: "accepted" | "acknowledged";
}

export interface ObserverPollView {
  id: string;
  owner_display: string;
  name: string;
  custom_prompt: string | null;
  dimensions: string[];
}

export interface OwnerResponseView {
  id: string;
  member_id: string;
  observer_email: string;
  date: string;
  scores: Record<string, number>;
  created_at: string;
}

export interface ObserverResponseView {
  id: string;
  date: string;
  scores: Record<string, number>;
  created_at: string;
}

export interface SubmitResponseRequest {
  date: string;
  scores: Record<string, number>;
}

export const observerPollsApi = {
  // Owner endpoints
  create: (data: CreatePollRequest) =>
    api.post<Poll>("/api/v1/observer-polls", data),

  list: () => api.get<Poll[]>("/api/v1/observer-polls"),

  get: (id: string) => api.get<Poll>(`/api/v1/observer-polls/${id}`),

  update: (id: string, data: UpdatePollRequest) =>
    api.patch<Poll>(`/api/v1/observer-polls/${id}`, data),

  delete: (id: string) =>
    api.delete<void>(`/api/v1/observer-polls/${id}`),

  invite: (pollId: string) =>
    api.post<InviteResponse>(
      `/api/v1/observer-polls/${pollId}/invite`,
      {},
    ),

  getResponses: (
    pollId: string,
    params?: { start?: string; end?: string },
  ) => {
    const searchParams = new URLSearchParams();
    if (params?.start) searchParams.set("start", params.start);
    if (params?.end) searchParams.set("end", params.end);
    const qs = searchParams.toString();
    const path = `/api/v1/observer-polls/${pollId}/responses${qs ? `?${qs}` : ""}`;
    return api.get<{ responses: OwnerResponseView[] }>(path);
  },

  // Observer endpoints
  accept: (token: string) =>
    api.post<AcceptResponse>("/api/v1/observer-polls/accept", { token }),

  myPolls: () =>
    api.get<ObserverPollView[]>("/api/v1/observer-polls/my-polls"),

  respond: (pollId: string, data: SubmitResponseRequest) =>
    api.put<ObserverResponseView>(
      `/api/v1/observer-polls/${pollId}/respond`,
      data,
    ),

  myResponses: (pollId: string) =>
    api.get<{ responses: ObserverResponseView[] }>(
      `/api/v1/observer-polls/${pollId}/my-responses`,
    ),

  deleteResponse: (responseId: string) =>
    api.delete<void>(`/api/v1/observer-polls/responses/${responseId}`),

  exportResponses: () =>
    api.get<{
      responses: Array<{
        poll_name: string;
        date: string;
        scores: Record<string, number>;
        created_at: string;
      }>;
    }>("/api/v1/observer-polls/export"),
};
