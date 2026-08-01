// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { type BackendEventSource, SOURCE_QUERY_KEYS, useSSE } from "../../src/hooks/useSSE";
import { useAuthStore } from "../../src/store/auth";

// The full set of sources the backend's `publish_event` is called with (see
// `backend/api/src/routes/*.rs`). Kept independent of `useSSE.ts` so the
// drift-guard test below actually fails if a new backend source is added
// without a matching map entry.
const BACKEND_EVENT_SOURCES = [
  "health_records",
  "protocols",
  "interventions",
  "checkins",
  "labs",
  "observations",
  "genetics",
].sort();

class MockEventSource {
  static instances: MockEventSource[] = [];
  url: string;
  listeners: Record<string, ((e: MessageEvent) => void)[]> = {};
  onerror: (() => void) | null = null;
  close = vi.fn();

  constructor(url: string) {
    this.url = url;
    MockEventSource.instances.push(this);
  }

  addEventListener(type: string, cb: (e: MessageEvent) => void) {
    (this.listeners[type] ??= []).push(cb);
  }

  emit(type: string, data: unknown) {
    this.emitRaw(type, JSON.stringify(data));
  }

  emitRaw(type: string, raw: string) {
    for (const cb of this.listeners[type] ?? []) {
      cb({ data: raw } as MessageEvent);
    }
  }
}

function wrapper(queryClient: QueryClient) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
  };
}

function invalidatedKeysOf(spy: ReturnType<typeof vi.spyOn>): unknown[] {
  return spy.mock.calls.map((call) => (call[0] as { queryKey: unknown[] }).queryKey[0]);
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

  it("has a map entry for exactly the backend's publish_event source set (drift guard)", () => {
    expect(Object.keys(SOURCE_QUERY_KEYS).sort()).toEqual(BACKEND_EVENT_SOURCES);
  });

  it.each(Object.entries(SOURCE_QUERY_KEYS) as [BackendEventSource, readonly string[]][])(
    "maps %s events to %j",
    (source, expectedKeys) => {
      const queryClient = new QueryClient();
      const spy = vi.spyOn(queryClient, "invalidateQueries");

      renderHook(() => useSSE(), { wrapper: wrapper(queryClient) });
      const es = MockEventSource.instances[0];
      es.emit("data_changed", { source });

      const invalidatedKeys = invalidatedKeysOf(spy);
      for (const key of expectedKeys) {
        expect(invalidatedKeys).toContain(key);
      }
      // Always-invalidated regardless of source.
      expect(invalidatedKeys).toContain("explore-series");
      expect(invalidatedKeys).toContain("dashboard-summary");
      expect(invalidatedKeys).toContain("dashboard-sparklines");
    },
  );

  it("falls back to invalidating the raw source for an unknown source, plus the always-keys", () => {
    const queryClient = new QueryClient();
    const spy = vi.spyOn(queryClient, "invalidateQueries");

    renderHook(() => useSSE(), { wrapper: wrapper(queryClient) });
    const es = MockEventSource.instances[0];
    es.emit("data_changed", { source: "some_future_source" });

    const invalidatedKeys = invalidatedKeysOf(spy);
    expect(invalidatedKeys).toEqual(
      expect.arrayContaining([
        "some_future_source",
        "explore-series",
        "dashboard-summary",
        "dashboard-sparklines",
      ]),
    );
    expect(invalidatedKeys).toHaveLength(4);
  });

  it("ignores a payload that is not valid JSON, without throwing", () => {
    const queryClient = new QueryClient();
    const spy = vi.spyOn(queryClient, "invalidateQueries");

    renderHook(() => useSSE(), { wrapper: wrapper(queryClient) });
    const es = MockEventSource.instances[0];

    expect(() => es.emitRaw("data_changed", "not json")).not.toThrow();
    expect(spy).not.toHaveBeenCalled();
  });

  it("ignores a payload whose source is missing or not a string, without throwing", () => {
    const queryClient = new QueryClient();
    const spy = vi.spyOn(queryClient, "invalidateQueries");

    renderHook(() => useSSE(), { wrapper: wrapper(queryClient) });
    const es = MockEventSource.instances[0];

    expect(() => es.emit("data_changed", {})).not.toThrow();
    expect(() => es.emit("data_changed", { source: 123 })).not.toThrow();
    expect(() => es.emit("data_changed", null)).not.toThrow();
    expect(spy).not.toHaveBeenCalled();
  });

  it("closes the EventSource on unmount", () => {
    const queryClient = new QueryClient();

    const { unmount } = renderHook(() => useSSE(), { wrapper: wrapper(queryClient) });
    const es = MockEventSource.instances[0];

    unmount();

    expect(es.close).toHaveBeenCalledOnce();
  });
});
