// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { trackApiCall } from "../lib/telemetry";
import { useAuthStore } from "../store/auth";
import { refreshTokenOnce } from "./refresh";

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
    /** Seconds from the `Retry-After` response header, when the backend sent one (429s). */
    public retryAfterSeconds?: number,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

// A 401 from one of these means the credentials or refresh cookie
// themselves are bad (not an expired access token), so attempting a refresh
// would just loop. Other /api/v1/auth/* routes (methods, link/unlink, ...)
// are ordinary access-token-authenticated calls and go through the normal
// refresh-and-retry flow below.
const NO_REFRESH_PATHS: ReadonlySet<string> = new Set([
  "/api/v1/auth/login",
  "/api/v1/auth/register",
  "/api/v1/auth/refresh",
  "/api/v1/auth/google/login",
]);

async function request<T>(path: string, options: RequestInit = {}, isRetry = false): Promise<T> {
  const token = useAuthStore.getState().token;

  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    // Build version (git SHA, injected by Vite) so the backend can log which
    // client build issued each request and surface stale clients in Loki.
    "X-App-Version": __APP_VERSION__,
    ...((options.headers as Record<string, string>) ?? {}),
  };

  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  const method = (options.method ?? "GET").toUpperCase();
  const startedAt = performance.now();

  let response: Response;
  try {
    response = await fetch(path, {
      ...options,
      headers,
      credentials: "include",
    });
  } catch (err) {
    // Network-level failure (no HTTP status). Report as status 0 so the call is
    // still counted, then rethrow. Only endpoint/method/status/latency are sent
    // — never the error message or any request/response body.
    trackApiCall({
      endpoint: path,
      method,
      status: 0,
      latency_ms: performance.now() - startedAt,
    });
    throw err;
  }

  // Emit first-party `api_call` telemetry with non-identifying metadata only.
  // The endpoint is scrubbed of id-shaped segments inside trackApiCall; bodies
  // are never included. Gated by the user's opt-in inside trackApiCall.
  trackApiCall({
    endpoint: path,
    method,
    status: response.status,
    latency_ms: performance.now() - startedAt,
  });

  if (response.status === 401) {
    // Refresh-and-retry only applies once (not on the retry itself), for a
    // token-bearing session (an anonymous 401 can't be a session expiring),
    // and for endpoints where a refresh could plausibly fix it.
    if (!isRetry && token && !NO_REFRESH_PATHS.has(path)) {
      const result = await refreshTokenOnce();
      if (result.ok) {
        return request<T>(path, options, true);
      }
      // Only a refresh rejection that says the session itself is invalid
      // (401/403) should log the user out. A 429 (shared rate limit) or 5xx
      // (refresh endpoint down) is transient — surface the original request's
      // failure and let the next request try again.
      if (result.status === 401 || result.status === 403) {
        useAuthStore.getState().logout();
      }
      throw new ApiError(401, "Unauthorized");
    }
    useAuthStore.getState().logout();
    throw new ApiError(401, "Unauthorized");
  }

  if (!response.ok) {
    const body = await response.text();
    const retryAfterHeader = response.headers.get("retry-after");
    const retryAfterSeconds = retryAfterHeader ? Number(retryAfterHeader) : undefined;
    throw new ApiError(
      response.status,
      body,
      Number.isFinite(retryAfterSeconds) ? retryAfterSeconds : undefined,
    );
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return response.json() as Promise<T>;
}

export const api = {
  get: <T>(path: string) => request<T>(path),

  post: <T>(path: string, body: unknown) =>
    request<T>(path, {
      method: "POST",
      body: JSON.stringify(body),
    }),

  put: <T>(path: string, body: unknown) =>
    request<T>(path, {
      method: "PUT",
      body: JSON.stringify(body),
    }),

  patch: <T>(path: string, body: unknown) =>
    request<T>(path, {
      method: "PATCH",
      body: JSON.stringify(body),
    }),

  delete: <T>(path: string) => request<T>(path, { method: "DELETE" }),
};
