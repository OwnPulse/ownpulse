// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAuthStore } from "../../src/store/auth";

describe("api client", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: null, isAuthenticated: false });
    vi.restoreAllMocks();
  });

  it("attaches Authorization header when token set", async () => {
    useAuthStore.getState().login("my-jwt");

    const mockFetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.resolve({ data: "ok" }),
    });
    vi.stubGlobal("fetch", mockFetch);

    // Dynamic import to pick up the stubbed fetch
    const { api } = await import("../../src/api/client");
    await api.get("/api/v1/test");

    expect(mockFetch).toHaveBeenCalledOnce();
    const [, options] = mockFetch.mock.calls[0];
    expect(options.headers.Authorization).toBe("Bearer my-jwt");

    vi.unstubAllGlobals();
  });

  it("calls logout on 401", async () => {
    useAuthStore.getState().login("expired-token");

    const mockFetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 401,
      text: () => Promise.resolve("Unauthorized"),
    });
    vi.stubGlobal("fetch", mockFetch);

    const { api } = await import("../../src/api/client");

    await expect(api.get("/api/v1/protected")).rejects.toThrow("Unauthorized");
    expect(useAuthStore.getState().isAuthenticated).toBe(false);
    expect(useAuthStore.getState().token).toBeNull();

    vi.unstubAllGlobals();
  });

  it("throws ApiError on non-OK response", async () => {
    const mockFetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 500,
      text: () => Promise.resolve("Internal Server Error"),
    });
    vi.stubGlobal("fetch", mockFetch);

    const { api } = await import("../../src/api/client");

    await expect(api.get("/api/v1/broken")).rejects.toThrow("Internal Server Error");

    vi.unstubAllGlobals();
  });
});
