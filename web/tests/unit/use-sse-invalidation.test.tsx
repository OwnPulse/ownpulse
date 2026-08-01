// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useSSE } from "../../src/hooks/useSSE";
import { useAuthStore } from "../../src/store/auth";

class MockEventSource {
  static instances: MockEventSource[] = [];
  url: string;
  listeners: Record<string, ((e: MessageEvent) => void)[]> = {};
  onerror: (() => void) | null = null;

  constructor(url: string) {
    this.url = url;
    MockEventSource.instances.push(this);
  }

  addEventListener(type: string, cb: (e: MessageEvent) => void) {
    (this.listeners[type] ??= []).push(cb);
  }

  emit(type: string, data: unknown) {
    for (const cb of this.listeners[type] ?? []) {
      cb({ data: JSON.stringify(data) } as MessageEvent);
    }
  }

  close() {}
}

function wrapper(queryClient: QueryClient) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
  };
}

describe("useSSE data_changed invalidation map", () => {
  beforeEach(() => {
    MockEventSource.instances = [];
    // eslint-disable-next-line @typescript-eslint/no-explicit-any -- test double for the global EventSource API
    vi.stubGlobal("EventSource", MockEventSource as any);
    useAuthStore.getState().login("test-jwt");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
    vi.unstubAllGlobals();
  });

  it("maps health_records events to health-records and dashboard-sparklines query keys", () => {
    const queryClient = new QueryClient();
    const spy = vi.spyOn(queryClient, "invalidateQueries");

    renderHook(() => useSSE(), { wrapper: wrapper(queryClient) });
    const es = MockEventSource.instances[0];
    es.emit("data_changed", { source: "health_records" });

    const invalidatedKeys = spy.mock.calls.map((call) => (call[0] as { queryKey: unknown[] }).queryKey[0]);
    expect(invalidatedKeys).toContain("health-records");
    expect(invalidatedKeys).toContain("dashboard-sparklines");
    expect(invalidatedKeys).toContain("explore-series");
    expect(invalidatedKeys).toContain("dashboard-summary");
  });

  it("maps protocols events to protocols, todays-doses, active-runs, protocol-runs", () => {
    const queryClient = new QueryClient();
    const spy = vi.spyOn(queryClient, "invalidateQueries");

    renderHook(() => useSSE(), { wrapper: wrapper(queryClient) });
    const es = MockEventSource.instances[0];
    es.emit("data_changed", { source: "protocols" });

    const invalidatedKeys = spy.mock.calls.map((call) => (call[0] as { queryKey: unknown[] }).queryKey[0]);
    expect(invalidatedKeys).toContain("protocols");
    expect(invalidatedKeys).toContain("todays-doses");
    expect(invalidatedKeys).toContain("active-runs");
    expect(invalidatedKeys).toContain("protocol-runs");
  });

  it("falls back to invalidating the raw source for unknown sources", () => {
    const queryClient = new QueryClient();
    const spy = vi.spyOn(queryClient, "invalidateQueries");

    renderHook(() => useSSE(), { wrapper: wrapper(queryClient) });
    const es = MockEventSource.instances[0];
    es.emit("data_changed", { source: "some_future_source" });

    const invalidatedKeys = spy.mock.calls.map((call) => (call[0] as { queryKey: unknown[] }).queryKey[0]);
    expect(invalidatedKeys).toContain("some_future_source");
  });
});
