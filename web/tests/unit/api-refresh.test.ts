// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { refreshToken, refreshTokenOnce } from "../../src/api/refresh";
import { useAuthStore } from "../../src/store/auth";

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

describe("refreshToken", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: null, isAuthenticated: false, role: null });
  });

  it("returns ok:true and stores the new token on a 200", async () => {
    server.use(
      http.post("/api/v1/auth/refresh", () =>
        HttpResponse.json({ access_token: "fresh-token", token_type: "Bearer", expires_in: 3600 }),
      ),
    );

    const result = await refreshToken();

    expect(result).toEqual({ ok: true });
    expect(useAuthStore.getState().token).toBe("fresh-token");
    expect(useAuthStore.getState().isAuthenticated).toBe(true);
  });

  it("returns ok:false with the response status on a non-2xx response", async () => {
    server.use(
      http.post("/api/v1/auth/refresh", () => new HttpResponse("Forbidden", { status: 403 })),
    );

    const result = await refreshToken();

    expect(result).toEqual({ ok: false, status: 403 });
    expect(useAuthStore.getState().token).toBeNull();
  });

  it("returns ok:false with status 0 on a network-level failure", async () => {
    server.use(http.post("/api/v1/auth/refresh", () => HttpResponse.error()));

    const result = await refreshToken();

    expect(result).toEqual({ ok: false, status: 0 });
    expect(useAuthStore.getState().token).toBeNull();
  });

  it("returns ok:false when the response body is not valid JSON", async () => {
    server.use(
      http.post(
        "/api/v1/auth/refresh",
        () => new HttpResponse("not json", { status: 200, headers: { "Content-Type": "application/json" } }),
      ),
    );

    const result = await refreshToken();

    expect(result.ok).toBe(false);
    expect(useAuthStore.getState().token).toBeNull();
  });
});

describe("refreshTokenOnce", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: null, isAuthenticated: false, role: null });
  });

  it("shares one in-flight fetch across concurrent callers", async () => {
    let refreshCalls = 0;
    let resolveRefresh!: () => void;
    const held = new Promise<void>((resolve) => {
      resolveRefresh = resolve;
    });

    server.use(
      http.post("/api/v1/auth/refresh", async () => {
        refreshCalls += 1;
        await held;
        return HttpResponse.json({
          access_token: "fresh-token",
          token_type: "Bearer",
          expires_in: 3600,
        });
      }),
    );

    const first = refreshTokenOnce();
    const second = refreshTokenOnce();
    resolveRefresh();
    const [firstResult, secondResult] = await Promise.all([first, second]);

    expect(refreshCalls).toBe(1);
    expect(firstResult).toEqual({ ok: true });
    expect(secondResult).toEqual({ ok: true });
  });

  it("starts a fresh fetch after the in-flight one settles (proves the promise clears)", async () => {
    let refreshCalls = 0;

    server.use(
      http.post("/api/v1/auth/refresh", () => {
        refreshCalls += 1;
        return HttpResponse.json({
          access_token: "fresh-token",
          token_type: "Bearer",
          expires_in: 3600,
        });
      }),
    );

    await refreshTokenOnce();
    await refreshTokenOnce();

    expect(refreshCalls).toBe(2);
  });
});
