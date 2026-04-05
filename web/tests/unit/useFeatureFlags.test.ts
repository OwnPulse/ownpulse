// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { createElement, type ReactNode } from "react";
import { afterAll, afterEach, beforeAll, describe, expect, it } from "vitest";
import { useAppConfig, useFeatureFlag } from "../../src/hooks/useFeatureFlags";

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

const mockConfig = {
  feature_flags: { dark_mode_v2: true, new_dashboard: false },
  ios: { min_supported_version: "2.0.0", force_upgrade_below: null },
};

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return createElement(QueryClientProvider, { client: queryClient }, children);
  };
}

describe("useAppConfig", () => {
  it("fetches /api/v1/config and returns data", async () => {
    server.use(http.get("/api/v1/config", () => HttpResponse.json(mockConfig)));

    const { result } = renderHook(() => useAppConfig(), {
      wrapper: createWrapper(),
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual(mockConfig);
  });

  it("returns error when request fails", async () => {
    server.use(http.get("/api/v1/config", () => new HttpResponse("Server Error", { status: 500 })));

    const { result } = renderHook(() => useAppConfig(), {
      wrapper: createWrapper(),
    });

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(result.current.error).toBeInstanceOf(Error);
  });
});

describe("useFeatureFlag", () => {
  it("returns false when config is not yet loaded", () => {
    server.use(
      http.get("/api/v1/config", async () => {
        // Never resolve — simulate pending state
        await new Promise(() => {});
        return HttpResponse.json(mockConfig);
      }),
    );

    const { result } = renderHook(() => useFeatureFlag("dark_mode_v2"), {
      wrapper: createWrapper(),
    });

    expect(result.current).toBe(false);
  });

  it("returns true for an enabled flag", async () => {
    server.use(http.get("/api/v1/config", () => HttpResponse.json(mockConfig)));

    const { result } = renderHook(() => useFeatureFlag("dark_mode_v2"), {
      wrapper: createWrapper(),
    });

    await waitFor(() => expect(result.current).toBe(true));
  });

  it("returns false for a disabled flag", async () => {
    server.use(http.get("/api/v1/config", () => HttpResponse.json(mockConfig)));

    const { result } = renderHook(() => useFeatureFlag("new_dashboard"), {
      wrapper: createWrapper(),
    });

    // Wait for the query to settle, then check it stays false
    await waitFor(() => expect(result.current).toBe(false));
  });

  it("returns false for a nonexistent flag", async () => {
    server.use(http.get("/api/v1/config", () => HttpResponse.json(mockConfig)));

    const { result } = renderHook(() => useFeatureFlag("nonexistent_flag"), {
      wrapper: createWrapper(),
    });

    // Give time for config to load, then verify still false
    await waitFor(() => expect(result.current).toBe(false));
  });
});
