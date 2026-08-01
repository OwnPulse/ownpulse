// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useAuthStore } from "../store/auth";

export interface TokenResponse {
  access_token: string;
  token_type: string;
  expires_in: number;
}

/**
 * Calls the refresh endpoint using the httpOnly refresh cookie and, on
 * success, stores the new access token in memory via the auth store.
 *
 * Deliberately kept in its own module (rather than `api/auth.ts`) so
 * `api/client.ts` can import it for the coalesced 401 retry flow below
 * without creating an import cycle between `client.ts` and `auth.ts`
 * (`auth.ts` imports the `api` wrapper from `client.ts` for its other
 * endpoints). This module never imports `client.ts`.
 */
export async function refreshToken(): Promise<boolean> {
  try {
    const response = await fetch("/api/v1/auth/refresh", {
      method: "POST",
      credentials: "include",
    });
    if (!response.ok) return false;
    const data: TokenResponse = await response.json();
    useAuthStore.getState().login(data.access_token);
    return true;
  } catch {
    return false;
  }
}

/**
 * Single-flight wrapper: concurrent callers share one in-flight refresh
 * request instead of each firing their own, so N concurrent 401s trigger
 * exactly one call to `/api/v1/auth/refresh`.
 */
let inFlightRefresh: Promise<boolean> | null = null;

export function refreshTokenOnce(): Promise<boolean> {
  if (!inFlightRefresh) {
    inFlightRefresh = refreshToken().finally(() => {
      inFlightRefresh = null;
    });
  }
  return inFlightRefresh;
}
