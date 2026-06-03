// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { api } from "../../src/api/client";
import { __bufferLength, flush, setTelemetryEnabled } from "../../src/lib/telemetry";
import { useAuthStore } from "../../src/store/auth";

interface CapturedReport {
  events: Array<{
    type: string;
    device_id: string | null;
    payload: Record<string, unknown>;
    platform: string;
  }>;
}

let lastReport: CapturedReport | null = null;
let telemetryPosts = 0;

const server = setupServer(
  http.get("/api/v1/protocols/:id/runs", () => HttpResponse.json({ ok: true })),
  http.post("/api/v1/auth/login", () =>
    HttpResponse.json({ access_token: "t", token_type: "Bearer", expires_in: 3600 }),
  ),
  http.get("/api/v1/broken", () => new HttpResponse("boom", { status: 500 })),
  http.post("/api/v1/telemetry/report", async ({ request }) => {
    lastReport = (await request.json()) as CapturedReport;
    telemetryPosts += 1;
    return HttpResponse.json({ accepted: lastReport.events.length, rejected: 0 });
  }),
);

beforeAll(() => server.listen());
afterAll(() => server.close());

beforeEach(() => {
  localStorage.clear();
  lastReport = null;
  telemetryPosts = 0;
  useAuthStore.getState().login("test-jwt-token");
});

afterEach(() => {
  server.resetHandlers();
  setTelemetryEnabled(false);
  useAuthStore.getState().logout();
  localStorage.clear();
});

describe("client api_call telemetry", () => {
  it("records a scrubbed api_call for a successful request when enabled", async () => {
    setTelemetryEnabled(true);
    await api.get("/api/v1/protocols/42/runs");
    await flush();

    const apiCalls = lastReport?.events.filter((e) => e.type === "api_call") ?? [];
    expect(apiCalls).toHaveLength(1);
    const payload = apiCalls[0].payload;
    expect(payload.endpoint).toBe("/api/:id/protocols/:id/runs");
    expect(payload.method).toBe("GET");
    expect(payload.status).toBe(200);
    expect(typeof payload.latency_ms).toBe("number");
    // No request/response body fields ever appear.
    expect(payload).not.toHaveProperty("body");
    expect(payload).not.toHaveProperty("response");
    // api_call events carry no device id.
    expect(apiCalls[0].device_id).toBeNull();
  });

  it("records an api_call with the error status on a failed request", async () => {
    setTelemetryEnabled(true);
    await expect(api.get("/api/v1/broken")).rejects.toThrow("boom");
    await flush();

    const apiCalls = lastReport?.events.filter((e) => e.type === "api_call") ?? [];
    expect(apiCalls).toHaveLength(1);
    expect(apiCalls[0].payload.status).toBe(500);
  });

  it("does not record api_call events when telemetry is disabled", async () => {
    await api.get("/api/v1/protocols/42/runs");
    expect(__bufferLength()).toBe(0);
  });

  it("flushing telemetry does not itself generate an api_call (no recursion)", async () => {
    setTelemetryEnabled(true);
    await api.get("/api/v1/protocols/42/runs");
    await flush();
    expect(telemetryPosts).toBe(1);
    // The flush POST must not have been buffered as another api_call.
    expect(__bufferLength()).toBe(0);
  });
});
