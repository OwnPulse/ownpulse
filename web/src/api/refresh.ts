// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useAuthStore } from "../store/auth";

export interface TokenResponse {
  access_token: string;
  token_type: string;
  expires_in: number;
}

/**
 * `status` is 0 for a network-level failure (no HTTP response at all), so
 * callers can distinguish "the refresh cookie is invalid" (401/403) from
 * "the refresh endpoint is unreachable/overloaded" (network error, 429,
 * 5xx) — only the former should force a logout.
 */
export type RefreshResult = { ok: true } | { ok: false; status: number };

// This module never imports `client.ts`. `client.ts` needs this function for
// the coalesced 401 retry flow, and `auth.ts` already imports the `api`
// wrapper from `client.ts` for its other endpoints — putting refresh here
// keeps `client.ts -> refresh.ts` and `auth.ts -> client.ts` from becoming a
// cycle.
export async function refreshToken(): Promise<RefreshResult> {
  let response: Response;
  try {
    response = await fetch("/api/v1/auth/refresh", {
      method: "POST",
      credentials: "include",
    });
  } catch {
    return { ok: false, status: 0 };
  }

  if (!response.ok) {
    // Status only — the body may echo request details we don't want in logs.
    console.warn(`refresh token request failed with status ${response.status}`);
    return { ok: false, status: response.status };
  }

  try {
    const data: TokenResponse = await response.json();
    useAuthStore.getState().login(data.access_token);
    return { ok: true };
  } catch {
    console.warn(`refresh token response (status ${response.status}) was not valid JSON`);
    return { ok: false, status: response.status };
  }
}

// Single-flight: concurrent callers share one in-flight refresh request
// instead of each rotating the refresh cookie independently. The backend
// tolerates same-tab races via its rotation grace window, but one request
// is still cheaper and avoids burning the window on self-races.
let inFlightRefresh: Promise<RefreshResult> | null = null;

export function refreshTokenOnce(): Promise<RefreshResult> {
  if (!inFlightRefresh) {
    inFlightRefresh = refreshToken().finally(() => {
      inFlightRefresh = null;
    });
  }
  return inFlightRefresh;
}
