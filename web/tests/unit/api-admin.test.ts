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

const mockUsers = [
  {
    id: "u1",
    email: "admin@example.com",
    username: "admin",
    auth_provider: "password",
    role: "admin",
    status: "active",
    data_region: "us",
    created_at: "2025-01-01T00:00:00Z",
  },
  {
    id: "u2",
    email: "user@example.com",
    auth_provider: "google",
    role: "user",
    status: "active",
    data_region: "us",
    created_at: "2025-06-01T00:00:00Z",
  },
];

const mockInvites = [
  {
    id: "inv1",
    code: "INVITE-ABC",
    label: "For friends",
    max_uses: 10,
    use_count: 3,
    expires_at: null,
    revoked_at: null,
    created_at: "2025-01-01T00:00:00Z",
  },
];

describe("adminApi", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: TOKEN, isAuthenticated: true });
  });

  describe("listUsers", () => {
    it("GETs /api/v1/admin/users and returns user list", async () => {
      server.use(
        http.get("/api/v1/admin/users", () => {
          return HttpResponse.json(mockUsers);
        }),
      );

      const { adminApi } = await import("../../src/api/admin");
      const result = await adminApi.listUsers();

      expect(result).toEqual(mockUsers);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.get("/api/v1/admin/users", () => new HttpResponse("Unauthorized", { status: 401 })),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.listUsers()).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.get(
          "/api/v1/admin/users",
          () => new HttpResponse("Internal Server Error", { status: 500 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.listUsers()).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });

    it("throws ApiError on 403", async () => {
      server.use(
        http.get("/api/v1/admin/users", () => new HttpResponse("Forbidden", { status: 403 })),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.listUsers()).rejects.toMatchObject({
        name: "ApiError",
        status: 403,
      });
    });
  });

  describe("updateUserStatus", () => {
    it("PATCHes /api/v1/admin/users/:id/status with correct body", async () => {
      let capturedBody: unknown;
      let capturedUserId: string | undefined;

      server.use(
        http.patch("/api/v1/admin/users/:userId/status", async ({ params, request }) => {
          capturedUserId = params.userId as string;
          capturedBody = await request.json();
          return HttpResponse.json({ ...mockUsers[1], status: "disabled" });
        }),
      );

      const { adminApi } = await import("../../src/api/admin");
      const result = await adminApi.updateUserStatus("u2", "disabled");

      expect(capturedUserId).toBe("u2");
      expect(capturedBody).toEqual({ status: "disabled" });
      expect(result.status).toBe("disabled");
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.patch(
          "/api/v1/admin/users/:userId/status",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.updateUserStatus("u2", "disabled")).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.patch(
          "/api/v1/admin/users/:userId/status",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.updateUserStatus("u2", "disabled")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("deleteUser", () => {
    it("DELETEs /api/v1/admin/users/:id", async () => {
      let capturedUserId: string | undefined;

      server.use(
        http.delete("/api/v1/admin/users/:userId", ({ params }) => {
          capturedUserId = params.userId as string;
          return HttpResponse.json(null);
        }),
      );

      const { adminApi } = await import("../../src/api/admin");
      await adminApi.deleteUser("u2");

      expect(capturedUserId).toBe("u2");
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.delete(
          "/api/v1/admin/users/:userId",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.deleteUser("u2")).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.delete(
          "/api/v1/admin/users/:userId",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.deleteUser("u2")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("listInvites", () => {
    it("GETs /api/v1/admin/invites and returns invite list", async () => {
      server.use(
        http.get("/api/v1/admin/invites", () => {
          return HttpResponse.json(mockInvites);
        }),
      );

      const { adminApi } = await import("../../src/api/admin");
      const result = await adminApi.listInvites();

      expect(result).toEqual(mockInvites);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.get(
          "/api/v1/admin/invites",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.listInvites()).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.get(
          "/api/v1/admin/invites",
          () => new HttpResponse("Internal Server Error", { status: 500 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.listInvites()).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("createInvite", () => {
    it("POSTs /api/v1/admin/invites with correct body", async () => {
      let capturedBody: unknown;
      const createdInvite = {
        id: "inv-new",
        code: "INVITE-NEW",
        label: "Test invite",
        max_uses: 5,
        use_count: 0,
        expires_at: null,
        revoked_at: null,
        created_at: "2026-03-22T00:00:00Z",
      };

      server.use(
        http.post("/api/v1/admin/invites", async ({ request }) => {
          capturedBody = await request.json();
          return HttpResponse.json(createdInvite);
        }),
      );

      const { adminApi } = await import("../../src/api/admin");
      const result = await adminApi.createInvite({
        label: "Test invite",
        max_uses: 5,
        expires_in_hours: 48,
      });

      expect(capturedBody).toEqual({
        label: "Test invite",
        max_uses: 5,
        expires_in_hours: 48,
      });
      expect(result).toEqual(createdInvite);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.post(
          "/api/v1/admin/invites",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.createInvite({ label: "test" })).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.post(
          "/api/v1/admin/invites",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.createInvite({ label: "test" })).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("revokeInvite", () => {
    it("DELETEs /api/v1/admin/invites/:id", async () => {
      let capturedId: string | undefined;
      const revokedInvite = { ...mockInvites[0], revoked_at: "2026-03-22T00:00:00Z" };

      server.use(
        http.delete("/api/v1/admin/invites/:id", ({ params }) => {
          capturedId = params.id as string;
          return HttpResponse.json(revokedInvite);
        }),
      );

      const { adminApi } = await import("../../src/api/admin");
      const result = await adminApi.revokeInvite("inv1");

      expect(capturedId).toBe("inv1");
      expect(result.revoked_at).toBe("2026-03-22T00:00:00Z");
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.delete(
          "/api/v1/admin/invites/:id",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.revokeInvite("inv1")).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.delete(
          "/api/v1/admin/invites/:id",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.revokeInvite("inv1")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("updateRole", () => {
    it("PATCHes /api/v1/admin/users/:id/role with correct body", async () => {
      let capturedBody: unknown;
      let capturedUserId: string | undefined;

      server.use(
        http.patch("/api/v1/admin/users/:userId/role", async ({ params, request }) => {
          capturedUserId = params.userId as string;
          capturedBody = await request.json();
          return HttpResponse.json({ ...mockUsers[1], role: "admin" });
        }),
      );

      const { adminApi } = await import("../../src/api/admin");
      const result = await adminApi.updateRole("u2", "admin");

      expect(capturedUserId).toBe("u2");
      expect(capturedBody).toEqual({ role: "admin" });
      expect(result.role).toBe("admin");
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.patch(
          "/api/v1/admin/users/:userId/role",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.updateRole("u2", "admin")).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.patch(
          "/api/v1/admin/users/:userId/role",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { adminApi } = await import("../../src/api/admin");

      await expect(adminApi.updateRole("u2", "admin")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });
});
