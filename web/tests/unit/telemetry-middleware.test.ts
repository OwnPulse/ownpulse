// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { create } from "zustand";
import { flush, setTelemetryEnabled } from "../../src/lib/telemetry";
import { telemetry } from "../../src/lib/telemetryMiddleware";
import { useAuthStore } from "../../src/store/auth";

interface CapturedReport {
  events: Array<{ type: string; payload: Record<string, unknown> }>;
}

let lastReport: CapturedReport | null = null;

const server = setupServer(
  http.post("/api/v1/telemetry/report", async ({ request }) => {
    lastReport = (await request.json()) as CapturedReport;
    return HttpResponse.json({ accepted: lastReport.events.length, rejected: 0 });
  }),
);

beforeAll(() => server.listen());
afterAll(() => server.close());

beforeEach(() => {
  localStorage.clear();
  lastReport = null;
  useAuthStore.getState().login("test-jwt-token");
});

afterEach(() => {
  server.resetHandlers();
  setTelemetryEnabled(false);
  useAuthStore.getState().logout();
  localStorage.clear();
});

interface CounterState {
  count: number;
  bump: () => void;
  bumpSilently: () => void;
}

function makeStore() {
  return create<CounterState>()(
    telemetry((set) => ({
      count: 0,
      bump: () => set((s) => ({ count: s.count + 1 }), false, "counter/bump"),
      bumpSilently: () => set((s) => ({ count: s.count + 1 })),
    })),
  );
}

describe("zustand telemetry middleware", () => {
  it("emits a flow event for a labeled mutation when enabled", async () => {
    setTelemetryEnabled(true);
    const store = makeStore();
    store.getState().bump();
    expect(store.getState().count).toBe(1);
    await flush();
    const flows = lastReport?.events.filter((e) => e.type === "flow") ?? [];
    expect(flows).toHaveLength(1);
    expect(flows[0].payload.flow).toBe("counter/bump");
    expect(flows[0].payload.outcome).toBe("completed");
  });

  it("does not emit for an unlabeled mutation", async () => {
    setTelemetryEnabled(true);
    const store = makeStore();
    store.getState().bumpSilently();
    expect(store.getState().count).toBe(1);
    await flush();
    expect(lastReport).toBeNull();
  });

  it("does not emit when telemetry is disabled", async () => {
    const store = makeStore();
    store.getState().bump();
    await flush();
    expect(lastReport).toBeNull();
  });

  it("still applies the state update (middleware is transparent)", () => {
    const store = makeStore();
    store.getState().bump();
    store.getState().bump();
    expect(store.getState().count).toBe(2);
  });
});
