// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { useAuthStore } from "../../src/store/auth";

const TOKEN = "test-jwt";

const server = setupServer();

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

const mockFlags = [
  {
    id: "f1",
    key: "dark_mode_v2",
    enabled: true,
    description: "Enable dark mode v2",
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
  },
  {
    id: "f2",
    key: "new_dashboard",
    enabled: false,
    description: null,
    created_at: "2026-02-01T00:00:00Z",
    updated_at: "2026-02-01T00:00:00Z",
  },
];

describe("adminApi feature flags", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: TOKEN, isAuthenticated: true });
  });

  describe("listFeatureFlags", () => {
    it("GETs /api/v1/admin/feature-flags and returns flag list", async () => {
      server.use(
        http.get("/api/v1/admin/feature-flags", () => HttpResponse.json(mockFlags)),
      );

      const { adminApi } = await import("../../src/api/admin");
      const result = await adminApi.listFeatureFlags();

      expect(result).toEqual(mockFlags);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.get(
          "/api/v1/admin/feature-flags",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.listFeatureFlags()).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 403", async () => {
      server.use(
        http.get(
          "/api/v1/admin/feature-flags",
          () => new HttpResponse("Forbidden", { status: 403 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.listFeatureFlags()).rejects.toMatchObject({
        name: "ApiError",
        status: 403,
      });
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.get(
          "/api/v1/admin/feature-flags",
          () => new HttpResponse("Internal Server Error", { status: 500 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.listFeatureFlags()).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("upsertFeatureFlag", () => {
    it("PUTs /api/v1/admin/feature-flags/:key with correct body", async () => {
      let capturedKey: string | undefined;
      let capturedBody: unknown;

      server.use(
        http.put("/api/v1/admin/feature-flags/:key", async ({ params, request }) => {
          capturedKey = params.key as string;
          capturedBody = await request.json();
          return HttpResponse.json({ ...mockFlags[0], enabled: false });
        }),
      );

      const { adminApi } = await import("../../src/api/admin");
      const result = await adminApi.upsertFeatureFlag("dark_mode_v2", {
        enabled: false,
        description: "Updated",
      });

      expect(capturedKey).toBe("dark_mode_v2");
      expect(capturedBody).toEqual({ enabled: false, description: "Updated" });
      expect(result.enabled).toBe(false);
    });

    it("encodes special characters in the key", async () => {
      let capturedUrl: string | undefined;

      server.use(
        http.put("/api/v1/admin/feature-flags/:key", ({ request }) => {
          capturedUrl = new URL(request.url).pathname;
          return HttpResponse.json(mockFlags[0]);
        }),
      );

      const { adminApi } = await import("../../src/api/admin");
      await adminApi.upsertFeatureFlag("flag/with spaces", { enabled: true });

      expect(capturedUrl).toBe("/api/v1/admin/feature-flags/flag%2Fwith%20spaces");
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.put(
          "/api/v1/admin/feature-flags/:key",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(
        adminApi.upsertFeatureFlag("test", { enabled: true }),
      ).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.put(
          "/api/v1/admin/feature-flags/:key",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(
        adminApi.upsertFeatureFlag("test", { enabled: true }),
      ).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("deleteFeatureFlag", () => {
    it("DELETEs /api/v1/admin/feature-flags/:key", async () => {
      let capturedKey: string | undefined;

      server.use(
        http.delete("/api/v1/admin/feature-flags/:key", ({ params }) => {
          capturedKey = params.key as string;
          return new HttpResponse(null, { status: 204 });
        }),
      );

      const { adminApi } = await import("../../src/api/admin");
      await adminApi.deleteFeatureFlag("dark_mode_v2");

      expect(capturedKey).toBe("dark_mode_v2");
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.delete(
          "/api/v1/admin/feature-flags/:key",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.deleteFeatureFlag("test")).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.delete(
          "/api/v1/admin/feature-flags/:key",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.deleteFeatureFlag("test")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });
});
