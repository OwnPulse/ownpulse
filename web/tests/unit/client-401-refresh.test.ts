// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { api, ApiError } from "../../src/api/client";
import { useAuthStore } from "../../src/store/auth";

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

describe("api client — coalesced 401 refresh-and-retry", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: "expired-token", isAuthenticated: true, role: null });
  });

  it("refreshes and retries a 401 on a non-auth endpoint, without logging out", async () => {
    let protectedCalls = 0;
    let refreshCalls = 0;

    server.use(
      http.get("/api/v1/protected", ({ request }) => {
        protectedCalls += 1;
        const auth = request.headers.get("Authorization");
        if (auth === "Bearer expired-token") {
          return new HttpResponse("Unauthorized", { status: 401 });
        }
        return HttpResponse.json({ data: "ok" });
      }),
      http.post("/api/v1/auth/refresh", () => {
        refreshCalls += 1;
        return HttpResponse.json({
          access_token: "fresh-token",
          token_type: "Bearer",
          expires_in: 3600,
        });
      }),
    );

    const result = await api.get<{ data: string }>("/api/v1/protected");

    expect(result).toEqual({ data: "ok" });
    expect(protectedCalls).toBe(2); // original 401 + retry with fresh token
    expect(refreshCalls).toBe(1);
    expect(useAuthStore.getState().isAuthenticated).toBe(true);
    expect(useAuthStore.getState().token).toBe("fresh-token");
  });

  it("shares a single in-flight refresh across N concurrent 401s", async () => {
    let refreshCalls = 0;

    server.use(
      http.get("/api/v1/protected", ({ request }) => {
        const auth = request.headers.get("Authorization");
        if (auth === "Bearer expired-token") {
          return new HttpResponse("Unauthorized", { status: 401 });
        }
        return HttpResponse.json({ data: "ok" });
      }),
      http.post("/api/v1/auth/refresh", async () => {
        refreshCalls += 1;
        // Simulate network latency so concurrent callers overlap.
        await new Promise((resolve) => setTimeout(resolve, 10));
        return HttpResponse.json({
          access_token: "fresh-token",
          token_type: "Bearer",
          expires_in: 3600,
        });
      }),
    );

    const results = await Promise.all([
      api.get("/api/v1/protected"),
      api.get("/api/v1/protected"),
      api.get("/api/v1/protected"),
    ]);

    expect(results).toEqual([{ data: "ok" }, { data: "ok" }, { data: "ok" }]);
    expect(refreshCalls).toBe(1);
  });

  it("logs out when the refresh request fails", async () => {
    let refreshCalls = 0;

    server.use(
      http.get("/api/v1/protected", () => new HttpResponse("Unauthorized", { status: 401 })),
      http.post("/api/v1/auth/refresh", () => {
        refreshCalls += 1;
        return new HttpResponse("Unauthorized", { status: 401 });
      }),
    );

    await expect(api.get("/api/v1/protected")).rejects.toMatchObject({
      name: "ApiError",
      status: 401,
    });

    expect(refreshCalls).toBe(1);
    expect(useAuthStore.getState().isAuthenticated).toBe(false);
    expect(useAuthStore.getState().token).toBeNull();
  });

  it("logs out if the retried request 401s again after a successful refresh", async () => {
    let refreshCalls = 0;

    server.use(
      http.get("/api/v1/protected", () => new HttpResponse("Unauthorized", { status: 401 })),
      http.post("/api/v1/auth/refresh", () => {
        refreshCalls += 1;
        return HttpResponse.json({
          access_token: "fresh-token",
          token_type: "Bearer",
          expires_in: 3600,
        });
      }),
    );

    await expect(api.get("/api/v1/protected")).rejects.toMatchObject({
      name: "ApiError",
      status: 401,
    });

    // Only one refresh attempt — the retry itself does not trigger another.
    expect(refreshCalls).toBe(1);
    expect(useAuthStore.getState().isAuthenticated).toBe(false);
    expect(useAuthStore.getState().token).toBeNull();
  });

  it("does not attempt a refresh for /api/v1/auth/* 401s (no refresh loop)", async () => {
    let refreshCalls = 0;

    server.use(
      http.get("/api/v1/auth/methods", () => new HttpResponse("Unauthorized", { status: 401 })),
      http.post("/api/v1/auth/refresh", () => {
        refreshCalls += 1;
        return new HttpResponse("Unauthorized", { status: 401 });
      }),
    );

    await expect(api.get("/api/v1/auth/methods")).rejects.toBeInstanceOf(ApiError);

    expect(refreshCalls).toBe(0);
    expect(useAuthStore.getState().isAuthenticated).toBe(false);
  });
});
