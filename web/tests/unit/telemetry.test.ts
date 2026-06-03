// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import {
  __bufferLength,
  flush,
  isTelemetryEnabled,
  resetDeviceId,
  scrubEndpoint,
  setTelemetryEnabled,
  trackAction,
  trackApiCall,
  trackPageView,
} from "../../src/lib/telemetry";
import { useAuthStore } from "../../src/store/auth";

const TELEMETRY_PATH = "/api/v1/telemetry/report";
const DEVICE_ID_KEY = "telemetry_device_id";

interface CapturedReport {
  events: Array<{
    type: string;
    device_id: string | null;
    payload: Record<string, unknown>;
    app_version: string | null;
    platform: string;
  }>;
}

let lastReport: CapturedReport | null = null;
let reportCount = 0;
let respondWith: { status: number } = { status: 200 };

const server = setupServer(
  http.post(TELEMETRY_PATH, async ({ request }) => {
    lastReport = (await request.json()) as CapturedReport;
    reportCount += 1;
    if (respondWith.status >= 400) {
      return new HttpResponse("error", { status: respondWith.status });
    }
    return HttpResponse.json({ accepted: lastReport.events.length, rejected: 0 });
  }),
);

beforeAll(() => server.listen());
afterAll(() => server.close());

beforeEach(() => {
  localStorage.clear();
  lastReport = null;
  reportCount = 0;
  respondWith = { status: 200 };
  resetDeviceId();
  // The flush path requires a JWT (mirrors the real client).
  useAuthStore.getState().login("test-jwt-token");
});

afterEach(() => {
  server.resetHandlers();
  useAuthStore.getState().logout();
  localStorage.clear();
});

describe("telemetry opt-in gate", () => {
  it("defaults to OFF", () => {
    expect(isTelemetryEnabled()).toBe(false);
  });

  it("does not buffer or send events when disabled", async () => {
    trackPageView("/dashboard");
    trackAction("settings/save");
    trackApiCall({ endpoint: "/account", method: "GET", status: 200, latency_ms: 12 });
    expect(__bufferLength()).toBe(0);
    await flush();
    expect(reportCount).toBe(0);
  });

  it("buffers and sends events once enabled", async () => {
    setTelemetryEnabled(true);
    trackPageView("/dashboard");
    expect(__bufferLength()).toBe(1);
    await flush();
    expect(reportCount).toBe(1);
    expect(lastReport?.events).toHaveLength(1);
    expect(lastReport?.events[0]).toMatchObject({ type: "screen", platform: "web" });
  });

  it("discards buffered events when disabled again", async () => {
    setTelemetryEnabled(true);
    trackPageView("/dashboard");
    expect(__bufferLength()).toBe(1);
    setTelemetryEnabled(false);
    expect(__bufferLength()).toBe(0);
    await flush();
    expect(reportCount).toBe(0);
  });
});

describe("platform tagging", () => {
  it("tags every event with platform=web", async () => {
    setTelemetryEnabled(true);
    trackPageView("/dashboard");
    trackAction("settings/save");
    trackApiCall({ endpoint: "/account", method: "GET", status: 200, latency_ms: 5 });
    await flush();
    expect(lastReport?.events.every((e) => e.platform === "web")).toBe(true);
  });
});

describe("anonymous device id", () => {
  it("attaches a stable anonymous device id to page_view and action events", async () => {
    setTelemetryEnabled(true);
    trackPageView("/dashboard");
    trackAction("settings/save");
    await flush();
    const ids = lastReport?.events.map((e) => e.device_id) ?? [];
    expect(ids[0]).toBeTruthy();
    expect(ids[0]).toBe(ids[1]); // same id within a session
    expect(localStorage.getItem(DEVICE_ID_KEY)).toBe(ids[0]);
  });

  it("resets the device id on resetDeviceId (logout) so sessions can't be correlated", async () => {
    setTelemetryEnabled(true);
    trackPageView("/dashboard");
    await flush();
    const firstId = lastReport?.events[0].device_id;
    expect(firstId).toBeTruthy();

    resetDeviceId();
    expect(localStorage.getItem(DEVICE_ID_KEY)).toBeNull();

    trackPageView("/explore");
    await flush();
    const secondId = lastReport?.events[0].device_id;
    expect(secondId).toBeTruthy();
    expect(secondId).not.toBe(firstId);
  });

  it("never attaches a device id to api_call events", async () => {
    setTelemetryEnabled(true);
    trackApiCall({ endpoint: "/account", method: "GET", status: 200, latency_ms: 7 });
    await flush();
    expect(lastReport?.events[0].device_id).toBeNull();
  });
});

describe("batching at 20", () => {
  it("auto-flushes when the buffer reaches 20 events", async () => {
    setTelemetryEnabled(true);
    for (let i = 0; i < 19; i++) {
      trackPageView("/dashboard");
    }
    expect(__bufferLength()).toBe(19);
    expect(reportCount).toBe(0);

    trackPageView("/dashboard"); // 20th — triggers flush
    // flush is async; wait a tick for the fetch to settle.
    await new Promise((r) => setTimeout(r, 0));
    expect(reportCount).toBe(1);
    expect(lastReport?.events).toHaveLength(20);
    expect(__bufferLength()).toBe(0);
  });
});

describe("api_call payload minimization", () => {
  it("sends only endpoint/method/status/latency/retry_count — no bodies", async () => {
    setTelemetryEnabled(true);
    trackApiCall({
      endpoint: "/health_records",
      method: "post",
      status: 201,
      latency_ms: 42.6,
      retry_count: 2,
    });
    await flush();
    const payload = lastReport?.events[0].payload ?? {};
    expect(Object.keys(payload).sort()).toEqual([
      "endpoint",
      "latency_ms",
      "method",
      "retry_count",
      "status",
    ]);
    expect(payload.method).toBe("POST"); // uppercased
    expect(payload.status).toBe(201);
    expect(payload.latency_ms).toBe(43); // rounded, non-negative
    expect(payload.retry_count).toBe(2);
    // No request/response body or other fields leak in.
    expect(payload).not.toHaveProperty("body");
    expect(payload).not.toHaveProperty("request_body");
  });

  it("defaults retry_count to 0 and clamps negatives", async () => {
    setTelemetryEnabled(true);
    trackApiCall({ endpoint: "/account", method: "GET", status: 200, latency_ms: -5 });
    await flush();
    const payload = lastReport?.events[0].payload ?? {};
    expect(payload.retry_count).toBe(0);
    expect(payload.latency_ms).toBe(0);
  });

  it("scrubs id-shaped path segments from api_call endpoints", async () => {
    setTelemetryEnabled(true);
    trackApiCall({
      endpoint: "/api/v1/protocols/550e8400-e29b-41d4-a716-446655440000/runs?token=secret",
      method: "GET",
      status: 200,
      latency_ms: 9,
    });
    await flush();
    expect(lastReport?.events[0].payload.endpoint).toBe("/api/:id/protocols/:id/runs");
  });
});

describe("scrubEndpoint mirrors the backend normalizer", () => {
  it("keeps static lowercase route words", () => {
    expect(scrubEndpoint("/account")).toBe("/account");
    expect(scrubEndpoint("/health_records")).toBe("/health_records");
  });

  it("collapses numeric, uuid, email, token, and mixed-case segments to :id", () => {
    expect(scrubEndpoint("/protocols/42/runs")).toBe("/protocols/:id/runs");
    expect(scrubEndpoint("/users/alice@example.com/profile")).toBe("/users/:id/profile");
    expect(scrubEndpoint("/users/jane-doe/profile")).toBe("/users/:id/profile");
    expect(scrubEndpoint("/records/550e8400e29b41d4a716446655440000")).toBe("/records/:id");
    expect(scrubEndpoint("/invite/YWxpY2VAZXhhbXBsZQ")).toBe("/invite/:id");
  });

  it("drops query strings and fragments", () => {
    expect(scrubEndpoint("/account?token=x")).toBe("/account");
    expect(scrubEndpoint("/account#section")).toBe("/account");
  });

  it("collapses absurdly long alphabetic segments", () => {
    expect(scrubEndpoint(`/x/${"a".repeat(40)}`)).toBe("/x/:id");
  });

  it("returns unknown for an empty endpoint", () => {
    expect(scrubEndpoint("")).toBe("unknown");
  });
});

describe("flush error handling", () => {
  it("does not throw on a 401 response", async () => {
    setTelemetryEnabled(true);
    respondWith = { status: 401 };
    trackPageView("/dashboard");
    await expect(flush()).resolves.toBeUndefined();
  });

  it("does not throw on a 500 response", async () => {
    setTelemetryEnabled(true);
    respondWith = { status: 500 };
    trackPageView("/dashboard");
    await expect(flush()).resolves.toBeUndefined();
  });

  it("does not send when there is no auth token", async () => {
    setTelemetryEnabled(true);
    useAuthStore.getState().logout();
    trackPageView("/dashboard");
    await flush();
    expect(reportCount).toBe(0);
  });

  it("does nothing when the buffer is empty", async () => {
    setTelemetryEnabled(true);
    await flush();
    expect(reportCount).toBe(0);
  });
});
