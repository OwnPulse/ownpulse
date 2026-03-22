// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { describe, it, expect, vi, beforeEach } from "vitest";
import { useAuthStore } from "../../src/store/auth";

const TOKEN = "test-jwt";

function mockFetch(status: number, body: unknown) {
  return vi.fn().mockResolvedValue({
    ok: status >= 200 && status < 300,
    status,
    json: () => Promise.resolve(body),
    text: () => Promise.resolve(JSON.stringify(body)),
  });
}

describe("auth multi-provider API", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: TOKEN, isAuthenticated: true });
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  it("loginWithApple POSTs id_token and platform", async () => {
    const fetchMock = mockFetch(204, null);
    vi.stubGlobal("fetch", fetchMock);

    const { loginWithApple } = await import("../../src/api/auth");
    await loginWithApple("apple-id-token-abc");

    expect(fetchMock).toHaveBeenCalledOnce();
    const [url, opts] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/v1/auth/apple/callback");
    expect(opts.method).toBe("POST");
    const body = JSON.parse(opts.body as string);
    expect(body).toEqual({ id_token: "apple-id-token-abc", platform: "web" });
  });

  it("getAuthMethods GETs /api/v1/auth/methods", async () => {
    const methods = [
      { id: "1", provider: "google", email: "user@example.com", created_at: "2026-01-01T00:00:00Z" },
    ];
    const fetchMock = mockFetch(200, methods);
    vi.stubGlobal("fetch", fetchMock);

    const { getAuthMethods } = await import("../../src/api/auth");
    const result = await getAuthMethods();

    expect(fetchMock).toHaveBeenCalledOnce();
    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/v1/auth/methods");
    expect(result).toEqual(methods);
  });

  it("linkAuth POSTs to /api/v1/auth/link", async () => {
    const updated = [
      { id: "1", provider: "google", email: "user@example.com", created_at: "2026-01-01T00:00:00Z" },
      { id: "2", provider: "apple", email: null, created_at: "2026-03-01T00:00:00Z" },
    ];
    const fetchMock = mockFetch(200, updated);
    vi.stubGlobal("fetch", fetchMock);

    const { linkAuth } = await import("../../src/api/auth");
    const result = await linkAuth({ provider: "apple", id_token: "tok" });

    expect(fetchMock).toHaveBeenCalledOnce();
    const [url, opts] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/v1/auth/link");
    expect(opts.method).toBe("POST");
    const body = JSON.parse(opts.body as string);
    expect(body).toEqual({ provider: "apple", id_token: "tok" });
    expect(result).toEqual(updated);
  });

  it("unlinkAuth DELETEs /api/v1/auth/link/:provider", async () => {
    const remaining = [
      { id: "1", provider: "google", email: "user@example.com", created_at: "2026-01-01T00:00:00Z" },
    ];
    const fetchMock = mockFetch(200, remaining);
    vi.stubGlobal("fetch", fetchMock);

    const { unlinkAuth } = await import("../../src/api/auth");
    const result = await unlinkAuth("apple");

    expect(fetchMock).toHaveBeenCalledOnce();
    const [url, opts] = fetchMock.mock.calls[0];
    expect(url).toBe("/api/v1/auth/link/apple");
    expect(opts.method).toBe("DELETE");
    expect(result).toEqual(remaining);
  });
});
