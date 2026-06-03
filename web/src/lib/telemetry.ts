// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

// First-party, opt-in web telemetry.
//
// Mirrors the iOS telemetry client: events are only sent when the user has
// explicitly opted in (default OFF). An anonymous device id (a random UUID held
// in localStorage) is attached to navigation/action events so the backend can
// bucket flows without ever learning user identity; it RESETS on logout so a
// new session can't be correlated with the previous one. Events are buffered
// and flushed to our own backend in batches of 20 — never to any third party.
//
// No health data is ever collected here: the only fields sent are navigation
// paths (collapsed to static route words), coarse action names, and
// non-identifying request metadata (endpoint/method/status/latency/retry).

import { useAuthStore } from "../store/auth";

const TELEMETRY_ENDPOINT = "/api/v1/telemetry/report";
const ENABLED_KEY = "telemetry_enabled";
const DEVICE_ID_KEY = "telemetry_device_id";
const BATCH_SIZE = 20;

/**
 * Wire event type sent to the backend. The backend's allowlist is
 * `crash | screen | flow | api_call`, so the web's conceptual events map onto
 * those: a page view is a `screen`, a user action is a `flow`.
 */
type WireEventType = "screen" | "flow" | "api_call";

interface TelemetryEvent {
  type: WireEventType;
  device_id: string | null;
  payload: Record<string, string | number>;
  app_version: string | null;
}

/** Non-identifying fields permitted on an `api_call` event. */
export interface ApiCallMeta {
  endpoint: string;
  method: string;
  status: number;
  latency_ms: number;
  retry_count?: number;
}

let buffer: TelemetryEvent[] = [];

function appVersion(): string | null {
  // Injected at build time by Vite's `define`. Falls back to null if absent.
  return typeof __APP_VERSION__ === "string" ? __APP_VERSION__ : null;
}

/** Whether telemetry is currently enabled. Default OFF when unset. */
export function isTelemetryEnabled(): boolean {
  if (typeof localStorage === "undefined") return false;
  return localStorage.getItem(ENABLED_KEY) === "true";
}

/** Enable or disable telemetry. Disabling discards any buffered events. */
export function setTelemetryEnabled(enabled: boolean): void {
  if (typeof localStorage === "undefined") return;
  if (enabled) {
    localStorage.setItem(ENABLED_KEY, "true");
  } else {
    localStorage.removeItem(ENABLED_KEY);
    buffer = [];
  }
}

/**
 * Return the anonymous device id, creating one on first use. The id is a random
 * UUID with no link to the account — it only groups events within one logged-in
 * session window. It is regenerated after logout via {@link resetDeviceId}.
 */
function getDeviceId(): string | null {
  if (typeof localStorage === "undefined") return null;
  let id = localStorage.getItem(DEVICE_ID_KEY);
  if (!id) {
    id = crypto.randomUUID();
    localStorage.setItem(DEVICE_ID_KEY, id);
  }
  return id;
}

/**
 * Clear the anonymous device id. Called from the logout flow so the next
 * session gets a fresh id and cannot be correlated with the previous one. Also
 * drops any buffered (unsent) events.
 */
export function resetDeviceId(): void {
  if (typeof localStorage === "undefined") return;
  localStorage.removeItem(DEVICE_ID_KEY);
  buffer = [];
}

/**
 * Collapse identifier-shaped path segments to `:id`, mirroring the backend's
 * `normalize_endpoint`. A segment is kept only if it is a short, lowercase,
 * underscore-only static route word; everything else (UUIDs, emails, tokens,
 * digits, mixed case, hyphens) becomes `:id`. Query strings and fragments are
 * dropped. This is the privacy-safe over-collapsing failure mode.
 */
export function scrubEndpoint(endpoint: string): string {
  const path = endpoint.split(/[?#]/)[0];
  if (path === "") return "unknown";
  return path
    .split("/")
    .map((seg) => (seg === "" || isRouteWord(seg) ? seg : ":id"))
    .join("/");
}

function isRouteWord(seg: string): boolean {
  return seg.length > 0 && seg.length <= 24 && /^[a-z_]+$/.test(seg);
}

function enqueue(event: TelemetryEvent): void {
  if (!isTelemetryEnabled()) return;
  buffer.push(event);
  if (buffer.length >= BATCH_SIZE) {
    void flush();
  }
}

/** Track a page view. `path` is scrubbed of identifier segments before sending. */
export function trackPageView(path: string): void {
  enqueue({
    type: "screen",
    device_id: getDeviceId(),
    payload: { screen: scrubEndpoint(path) },
    app_version: appVersion(),
  });
}

/**
 * Track a user action (e.g. a state mutation). `name` is a coarse, caller-chosen
 * label — never free-text user content.
 */
export function trackAction(name: string, outcome = "completed"): void {
  enqueue({
    type: "flow",
    device_id: getDeviceId(),
    payload: { flow: name, outcome },
    app_version: appVersion(),
  });
}

/**
 * Track an API call. Only non-identifying metadata is sent: a scrubbed
 * endpoint, the HTTP method, status, latency, and retry count. Request and
 * response bodies are never included. `api_call` events carry no device id —
 * the backend does not associate them with a device, and neither do we.
 */
export function trackApiCall(meta: ApiCallMeta): void {
  const payload: Record<string, string | number> = {
    endpoint: scrubEndpoint(meta.endpoint),
    method: meta.method.toUpperCase(),
    status: meta.status,
    latency_ms: Math.max(0, Math.round(meta.latency_ms)),
    retry_count: Math.max(0, Math.round(meta.retry_count ?? 0)),
  };
  enqueue({
    type: "api_call",
    device_id: null,
    payload,
    app_version: appVersion(),
  });
}

/**
 * Flush buffered events to the backend in a single POST. Sends directly via
 * `fetch` (not the `api` client) so that flushing telemetry does not itself
 * generate an `api_call` event and recurse. Requires an in-memory JWT; with no
 * token the events are dropped. On failure the events are dropped too —
 * telemetry is best-effort and must never block or retry-storm the app.
 */
export async function flush(): Promise<void> {
  if (buffer.length === 0) return;
  const token = useAuthStore.getState().token;
  if (!token) {
    buffer = [];
    return;
  }

  const events = buffer;
  buffer = [];

  try {
    await fetch(TELEMETRY_ENDPOINT, {
      method: "POST",
      credentials: "include",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify({ events: events.map((e) => ({ ...e, platform: "web" })) }),
    });
  } catch {
    // Best-effort: telemetry failures are silent and non-retrying.
  }
}

/** Test-only accessor for the pending buffer length. */
export function __bufferLength(): number {
  return buffer.length;
}
