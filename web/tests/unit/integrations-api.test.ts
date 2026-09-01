// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { integrationsApi } from "../../src/api/integrations";
import { useAuthStore } from "../../src/store/auth";

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

describe("integrationsApi", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: "test-jwt", isAuthenticated: true, role: null });
  });

  describe("list", () => {
    it("returns the parsed integration statuses on success", async () => {
      server.use(
        http.get("/api/v1/integrations", () =>
          HttpResponse.json([
            { source: "google_calendar", connected: true, last_synced_at: "2026-08-01T00:00:00Z" },
          ]),
        ),
      );

      const result = await integrationsApi.list();
      expect(result).toEqual([
        { source: "google_calendar", connected: true, last_synced_at: "2026-08-01T00:00:00Z" },
      ]);
    });

    it("rejects with a 401 ApiError and logs out an authenticated session when the refresh also 401s", async () => {
      // Exercises the real authenticated path (see client-401-refresh.test.ts):
      // a 401 from an in-session call triggers the client's refresh-and-retry,
      // and only logs out if the refresh itself confirms the session is dead.
      server.use(
        http.get("/api/v1/integrations", () => new HttpResponse("Unauthorized", { status: 401 })),
        http.post("/api/v1/auth/refresh", () => new HttpResponse("Unauthorized", { status: 401 })),
      );
      await expect(integrationsApi.list()).rejects.toMatchObject({
        name: "ApiError",
        status: 401,
      });
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
      expect(useAuthStore.getState().token).toBeNull();
    });

    it("rejects with an ApiError on 500", async () => {
      server.use(
        http.get("/api/v1/integrations", () => new HttpResponse("Error", { status: 500 })),
      );
      await expect(integrationsApi.list()).rejects.toMatchObject({ status: 500 });
    });
  });

  describe("disconnect", () => {
    it("resolves on a successful 204", async () => {
      server.use(
        http.delete(
          "/api/v1/integrations/google_calendar",
          () => new HttpResponse(null, { status: 204 }),
        ),
      );
      await expect(integrationsApi.disconnect("google_calendar")).resolves.toBeUndefined();
    });

    it("rejects with an ApiError on 403", async () => {
      server.use(
        http.delete(
          "/api/v1/integrations/google_calendar",
          () => new HttpResponse("Forbidden", { status: 403 }),
        ),
      );
      await expect(integrationsApi.disconnect("google_calendar")).rejects.toMatchObject({
        status: 403,
      });
    });

    it("rejects with an ApiError on 500", async () => {
      server.use(
        http.delete(
          "/api/v1/integrations/google_calendar",
          () => new HttpResponse("Error", { status: 500 }),
        ),
      );
      await expect(integrationsApi.disconnect("google_calendar")).rejects.toMatchObject({
        status: 500,
      });
    });
  });

  describe("sync", () => {
    it("returns the sync result on success", async () => {
      server.use(
        http.post("/api/v1/integrations/google-calendar/sync", () =>
          HttpResponse.json({ source: "google_calendar", records_inserted: 5 }),
        ),
      );
      const result = await integrationsApi.sync("google_calendar");
      expect(result).toEqual({ source: "google_calendar", records_inserted: 5 });
    });

    it("rejects with a 401 ApiError and logs out an authenticated session when the refresh also 401s", async () => {
      server.use(
        http.post(
          "/api/v1/integrations/google-calendar/sync",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
        http.post("/api/v1/auth/refresh", () => new HttpResponse("Unauthorized", { status: 401 })),
      );
      await expect(integrationsApi.sync("google_calendar")).rejects.toMatchObject({
        name: "ApiError",
        status: 401,
      });
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
      expect(useAuthStore.getState().token).toBeNull();
    });

    it("rejects with an ApiError carrying retryAfterSeconds on 429", async () => {
      server.use(
        http.post(
          "/api/v1/integrations/google-calendar/sync",
          () =>
            new HttpResponse(JSON.stringify({ error: "rate limited" }), {
              status: 429,
              headers: { "retry-after": "20" },
            }),
        ),
      );
      await expect(integrationsApi.sync("google_calendar")).rejects.toMatchObject({
        status: 429,
        retryAfterSeconds: 20,
      });
    });

    it("rejects with an ApiError on 500", async () => {
      server.use(
        http.post(
          "/api/v1/integrations/google-calendar/sync",
          () => new HttpResponse("Error", { status: 500 }),
        ),
      );
      await expect(integrationsApi.sync("google_calendar")).rejects.toMatchObject({
        status: 500,
      });
    });
  });
});
