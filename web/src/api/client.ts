// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { trackApiCall } from "../lib/telemetry";
import { useAuthStore } from "../store/auth";

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
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
    useAuthStore.getState().logout();
    throw new ApiError(401, "Unauthorized");
  }

  if (!response.ok) {
    const body = await response.text();
    throw new ApiError(response.status, body);
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
