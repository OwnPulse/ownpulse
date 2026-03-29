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

describe("invitesApi", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: TOKEN, isAuthenticated: true });
  });

  describe("check", () => {
    it("GETs /api/v1/invites/:code/check and returns valid invite", async () => {
      const validResponse = {
        valid: true,
        label: "For friends",
        expires_at: "2026-04-01T00:00:00Z",
        inviter_name: "Tony",
      };

      server.use(
        http.get("/api/v1/invites/:code/check", () => {
          return HttpResponse.json(validResponse);
        }),
      );

      const { invitesApi } = await import("../../src/api/invites");
      const result = await invitesApi.check("ABC123");

      expect(result).toEqual(validResponse);
    });

    it("returns invalid invite with reason", async () => {
      const invalidResponse = {
        valid: false,
        label: null,
        expires_at: null,
        inviter_name: null,
        reason: "expired",
      };

      server.use(
        http.get("/api/v1/invites/:code/check", () => {
          return HttpResponse.json(invalidResponse);
        }),
      );

      const { invitesApi } = await import("../../src/api/invites");
      const result = await invitesApi.check("EXPIRED-CODE");

      expect(result.valid).toBe(false);
      expect(result.reason).toBe("expired");
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.get(
          "/api/v1/invites/:code/check",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { invitesApi } = await import("../../src/api/invites");

      await expect(invitesApi.check("ABC123")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.get(
          "/api/v1/invites/:code/check",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { invitesApi } = await import("../../src/api/invites");

      await expect(invitesApi.check("ABC123")).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 403", async () => {
      server.use(
        http.get(
          "/api/v1/invites/:code/check",
          () => new HttpResponse("Forbidden", { status: 403 }),
        ),
      );

      const { invitesApi } = await import("../../src/api/invites");

      await expect(invitesApi.check("ABC123")).rejects.toMatchObject({
        name: "ApiError",
        status: 403,
      });
    });
  });

  describe("getClaims", () => {
    it("GETs /api/v1/admin/invites/:id/claims and returns claims", async () => {
      const claims = [
        { user_email: "alice@example.com", claimed_at: "2026-03-20T10:00:00Z" },
        { user_email: "bob@example.com", claimed_at: "2026-03-21T10:00:00Z" },
      ];

      server.use(
        http.get("/api/v1/admin/invites/:id/claims", () => {
          return HttpResponse.json(claims);
        }),
      );

      const { invitesApi } = await import("../../src/api/invites");
      const result = await invitesApi.getClaims("inv-1");

      expect(result).toEqual(claims);
      expect(result).toHaveLength(2);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.get(
          "/api/v1/admin/invites/:id/claims",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { invitesApi } = await import("../../src/api/invites");

      await expect(invitesApi.getClaims("inv-1")).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.get(
          "/api/v1/admin/invites/:id/claims",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { invitesApi } = await import("../../src/api/invites");

      await expect(invitesApi.getClaims("inv-1")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });

    it("throws ApiError on 403", async () => {
      server.use(
        http.get(
          "/api/v1/admin/invites/:id/claims",
          () => new HttpResponse("Forbidden", { status: 403 }),
        ),
      );

      const { invitesApi } = await import("../../src/api/invites");

      await expect(invitesApi.getClaims("inv-1")).rejects.toMatchObject({
        name: "ApiError",
        status: 403,
      });
    });
  });

  describe("sendEmail", () => {
    it("POSTs /api/v1/admin/invites/:id/send-email with correct body", async () => {
      let capturedBody: unknown;
      let capturedId: string | undefined;

      server.use(
        http.post("/api/v1/admin/invites/:id/send-email", async ({ params, request }) => {
          capturedId = params.id as string;
          capturedBody = await request.json();
          return new HttpResponse(null, { status: 204 });
        }),
      );

      const { invitesApi } = await import("../../src/api/invites");
      await invitesApi.sendEmail("inv-1", { email: "alice@example.com" });

      expect(capturedId).toBe("inv-1");
      expect(capturedBody).toEqual({ email: "alice@example.com" });
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.post(
          "/api/v1/admin/invites/:id/send-email",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { invitesApi } = await import("../../src/api/invites");

      await expect(invitesApi.sendEmail("inv-1", { email: "alice@example.com" })).rejects.toThrow(
        "Unauthorized",
      );
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.post(
          "/api/v1/admin/invites/:id/send-email",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { invitesApi } = await import("../../src/api/invites");

      await expect(
        invitesApi.sendEmail("inv-1", { email: "alice@example.com" }),
      ).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });
});
