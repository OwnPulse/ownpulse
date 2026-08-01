// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { ApiError, api } from "../../src/api/client";
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

  it("also refreshes and retries a 401 on /api/v1/auth/methods (access-token-authenticated, not a credentials route)", async () => {
    let methodsCalls = 0;
    let refreshCalls = 0;

    server.use(
      http.get("/api/v1/auth/methods", ({ request }) => {
        methodsCalls += 1;
        const auth = request.headers.get("Authorization");
        if (auth === "Bearer expired-token") {
          return new HttpResponse("Unauthorized", { status: 401 });
        }
        return HttpResponse.json([]);
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

    const result = await api.get("/api/v1/auth/methods");

    expect(result).toEqual([]);
    expect(methodsCalls).toBe(2);
    expect(refreshCalls).toBe(1);
    expect(useAuthStore.getState().isAuthenticated).toBe(true);
  });

  it("does not attempt a refresh for a 401 on /api/v1/auth/login (a credentials route)", async () => {
    let refreshCalls = 0;

    server.use(
      http.post("/api/v1/auth/login", () => new HttpResponse("Unauthorized", { status: 401 })),
      http.post("/api/v1/auth/refresh", () => {
        refreshCalls += 1;
        return new HttpResponse("Unauthorized", { status: 401 });
      }),
    );

    await expect(
      api.post("/api/v1/auth/login", { email: "a@example.com", password: "bad" }),
    ).rejects.toBeInstanceOf(ApiError);

    expect(refreshCalls).toBe(0);
    expect(useAuthStore.getState().isAuthenticated).toBe(false);
  });

  it("does not attempt a refresh for an anonymous 401 (no token in the store)", async () => {
    useAuthStore.setState({ token: null, isAuthenticated: false, role: null });
    let refreshCalls = 0;

    server.use(
      http.get("/api/v1/protected", () => new HttpResponse("Unauthorized", { status: 401 })),
      http.post("/api/v1/auth/refresh", () => {
        refreshCalls += 1;
        return new HttpResponse("Unauthorized", { status: 401 });
      }),
    );

    await expect(api.get("/api/v1/protected")).rejects.toBeInstanceOf(ApiError);

    expect(refreshCalls).toBe(0);
  });

  it("shares a single in-flight refresh across N concurrent 401s", async () => {
    let refreshCalls = 0;
    let pendingGets = 0;
    const totalGets = 3;
    let releaseRefresh!: () => void;
    const allGetsIn401 = new Promise<void>((resolve) => {
      releaseRefresh = resolve;
    });

    server.use(
      http.get("/api/v1/protected", ({ request }) => {
        const auth = request.headers.get("Authorization");
        if (auth === "Bearer expired-token") {
          pendingGets += 1;
          if (pendingGets === totalGets) releaseRefresh();
          return new HttpResponse("Unauthorized", { status: 401 });
        }
        return HttpResponse.json({ data: "ok" });
      }),
      http.post("/api/v1/auth/refresh", async () => {
        refreshCalls += 1;
        // Deterministic: resolve only once every concurrent GET has already
        // 401'd, proving they were all in-flight before the single refresh
        // completed (rather than relying on a fixed timer to "probably" win
        // the race).
        await allGetsIn401;
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

  it("logs out when the refresh request itself 401s", async () => {
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

  it("logs out when the refresh request 403s", async () => {
    server.use(
      http.get("/api/v1/protected", () => new HttpResponse("Unauthorized", { status: 401 })),
      http.post("/api/v1/auth/refresh", () => new HttpResponse("Forbidden", { status: 403 })),
    );

    await expect(api.get("/api/v1/protected")).rejects.toBeInstanceOf(ApiError);

    expect(useAuthStore.getState().isAuthenticated).toBe(false);
    expect(useAuthStore.getState().token).toBeNull();
  });

  it("does NOT log out when the refresh request 429s — fails the original request instead", async () => {
    server.use(
      http.get("/api/v1/protected", () => new HttpResponse("Unauthorized", { status: 401 })),
      http.post(
        "/api/v1/auth/refresh",
        () => new HttpResponse("Too Many Requests", { status: 429 }),
      ),
    );

    await expect(api.get("/api/v1/protected")).rejects.toMatchObject({
      name: "ApiError",
      status: 401,
    });

    // The user is still holding the (temporarily unusable) session — a
    // rate-limited refresh endpoint is not evidence the session is invalid.
    expect(useAuthStore.getState().isAuthenticated).toBe(true);
    expect(useAuthStore.getState().token).toBe("expired-token");
  });

  it("does NOT log out when the refresh request 500s — fails the original request instead", async () => {
    server.use(
      http.get("/api/v1/protected", () => new HttpResponse("Unauthorized", { status: 401 })),
      http.post(
        "/api/v1/auth/refresh",
        () => new HttpResponse("Internal Server Error", { status: 500 }),
      ),
    );

    await expect(api.get("/api/v1/protected")).rejects.toMatchObject({
      name: "ApiError",
      status: 401,
    });

    expect(useAuthStore.getState().isAuthenticated).toBe(true);
    expect(useAuthStore.getState().token).toBe("expired-token");
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
});
