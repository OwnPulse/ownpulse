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

const mockPoll = {
  id: "poll-1",
  name: "Daily mood check",
  custom_prompt: "How did I seem today?",
  dimensions: ["energy", "mood", "focus"],
  members: [
    {
      id: "member-1",
      observer_email: "s***@example.com",
      accepted_at: "2026-03-01T00:00:00Z",
      created_at: "2026-02-28T00:00:00Z",
    },
  ],
  created_at: "2026-02-28T00:00:00Z",
  deleted_at: null,
};

const mockObserverPoll = {
  id: "poll-1",
  owner_display: "J***",
  name: "Daily mood check",
  custom_prompt: "How did I seem today?",
  dimensions: ["energy", "mood"],
};

describe("observerPollsApi", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: TOKEN, isAuthenticated: true });
  });

  describe("list", () => {
    it("GETs /api/v1/observer-polls and returns poll list", async () => {
      server.use(
        http.get("/api/v1/observer-polls", () => HttpResponse.json([mockPoll])),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      const result = await observerPollsApi.list();

      expect(result).toEqual([mockPoll]);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.get("/api/v1/observer-polls", () => new HttpResponse("Unauthorized", { status: 401 })),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.list()).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.get(
          "/api/v1/observer-polls",
          () => new HttpResponse("Internal Server Error", { status: 500 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.list()).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });

    it("throws ApiError on 403", async () => {
      server.use(
        http.get("/api/v1/observer-polls", () => new HttpResponse("Forbidden", { status: 403 })),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.list()).rejects.toMatchObject({
        name: "ApiError",
        status: 403,
      });
    });
  });

  describe("create", () => {
    it("POSTs /api/v1/observer-polls with correct body", async () => {
      let capturedBody: unknown;

      server.use(
        http.post("/api/v1/observer-polls", async ({ request }) => {
          capturedBody = await request.json();
          return HttpResponse.json(mockPoll, { status: 201 });
        }),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      const result = await observerPollsApi.create({
        name: "Daily mood check",
        custom_prompt: "How did I seem today?",
        dimensions: ["energy", "mood", "focus"],
      });

      expect(capturedBody).toEqual({
        name: "Daily mood check",
        custom_prompt: "How did I seem today?",
        dimensions: ["energy", "mood", "focus"],
      });
      expect(result).toEqual(mockPoll);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.post(
          "/api/v1/observer-polls",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(
        observerPollsApi.create({ name: "test", dimensions: ["mood"] }),
      ).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.post(
          "/api/v1/observer-polls",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(
        observerPollsApi.create({ name: "test", dimensions: ["mood"] }),
      ).rejects.toMatchObject({ name: "ApiError", status: 500 });
    });
  });

  describe("get", () => {
    it("GETs /api/v1/observer-polls/:id", async () => {
      server.use(
        http.get("/api/v1/observer-polls/poll-1", () => HttpResponse.json(mockPoll)),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      const result = await observerPollsApi.get("poll-1");

      expect(result).toEqual(mockPoll);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.get(
          "/api/v1/observer-polls/poll-1",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.get("poll-1")).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.get(
          "/api/v1/observer-polls/poll-1",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.get("poll-1")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("update", () => {
    it("PATCHes /api/v1/observer-polls/:id with correct body", async () => {
      let capturedBody: unknown;

      server.use(
        http.patch("/api/v1/observer-polls/poll-1", async ({ request }) => {
          capturedBody = await request.json();
          return HttpResponse.json({ ...mockPoll, name: "Updated name" });
        }),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      const result = await observerPollsApi.update("poll-1", { name: "Updated name" });

      expect(capturedBody).toEqual({ name: "Updated name" });
      expect(result.name).toBe("Updated name");
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.patch(
          "/api/v1/observer-polls/poll-1",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(
        observerPollsApi.update("poll-1", { name: "test" }),
      ).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.patch(
          "/api/v1/observer-polls/poll-1",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(
        observerPollsApi.update("poll-1", { name: "test" }),
      ).rejects.toMatchObject({ name: "ApiError", status: 500 });
    });
  });

  describe("delete", () => {
    it("DELETEs /api/v1/observer-polls/:id", async () => {
      let capturedId = false;

      server.use(
        http.delete("/api/v1/observer-polls/poll-1", () => {
          capturedId = true;
          return HttpResponse.json(null);
        }),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await observerPollsApi.delete("poll-1");

      expect(capturedId).toBe(true);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.delete(
          "/api/v1/observer-polls/poll-1",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.delete("poll-1")).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.delete(
          "/api/v1/observer-polls/poll-1",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.delete("poll-1")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("invite", () => {
    it("POSTs /api/v1/observer-polls/:id/invite and returns invite data", async () => {
      const inviteData = {
        invite_token: "test-token",
        invite_expires_at: "2026-04-04T00:00:00Z",
        invite_url: "http://localhost/observe/accept?token=test-token",
      };

      server.use(
        http.post("/api/v1/observer-polls/poll-1/invite", () =>
          HttpResponse.json(inviteData, { status: 201 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      const result = await observerPollsApi.invite("poll-1");

      expect(result).toEqual(inviteData);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.post(
          "/api/v1/observer-polls/poll-1/invite",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.invite("poll-1")).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.post(
          "/api/v1/observer-polls/poll-1/invite",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.invite("poll-1")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("getResponses", () => {
    it("GETs /api/v1/observer-polls/:id/responses", async () => {
      const responseData = {
        responses: [
          {
            id: "resp-1",
            member_id: "member-1",
            observer_email: "s***@example.com",
            date: "2026-03-27",
            scores: { energy: 7, mood: 8 },
            created_at: "2026-03-27T10:00:00Z",
          },
        ],
      };

      server.use(
        http.get("/api/v1/observer-polls/poll-1/responses", () =>
          HttpResponse.json(responseData),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      const result = await observerPollsApi.getResponses("poll-1");

      expect(result).toEqual(responseData);
    });

    it("appends query params for start and end", async () => {
      let capturedUrl = "";

      server.use(
        http.get("/api/v1/observer-polls/poll-1/responses", ({ request }) => {
          capturedUrl = request.url;
          return HttpResponse.json({ responses: [] });
        }),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await observerPollsApi.getResponses("poll-1", {
        start: "2026-03-01",
        end: "2026-03-31",
      });

      expect(capturedUrl).toContain("start=2026-03-01");
      expect(capturedUrl).toContain("end=2026-03-31");
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.get(
          "/api/v1/observer-polls/poll-1/responses",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.getResponses("poll-1")).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.get(
          "/api/v1/observer-polls/poll-1/responses",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.getResponses("poll-1")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("accept", () => {
    it("POSTs /api/v1/observer-polls/accept with token", async () => {
      let capturedBody: unknown;

      server.use(
        http.post("/api/v1/observer-polls/accept", async ({ request }) => {
          capturedBody = await request.json();
          return HttpResponse.json({ status: "accepted" });
        }),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      const result = await observerPollsApi.accept("invite-token-123");

      expect(capturedBody).toEqual({ token: "invite-token-123" });
      expect(result.status).toBe("accepted");
    });

    it("returns acknowledged status", async () => {
      server.use(
        http.post("/api/v1/observer-polls/accept", () =>
          HttpResponse.json({ status: "acknowledged" }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      const result = await observerPollsApi.accept("expired-token");

      expect(result.status).toBe("acknowledged");
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.post(
          "/api/v1/observer-polls/accept",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.accept("token")).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.post(
          "/api/v1/observer-polls/accept",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.accept("token")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("myPolls", () => {
    it("GETs /api/v1/observer-polls/my-polls", async () => {
      server.use(
        http.get("/api/v1/observer-polls/my-polls", () =>
          HttpResponse.json([mockObserverPoll]),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      const result = await observerPollsApi.myPolls();

      expect(result).toEqual([mockObserverPoll]);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.get(
          "/api/v1/observer-polls/my-polls",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.myPolls()).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.get(
          "/api/v1/observer-polls/my-polls",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.myPolls()).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("respond", () => {
    it("PUTs /api/v1/observer-polls/:id/respond with correct body", async () => {
      let capturedBody: unknown;

      const responseView = {
        id: "resp-1",
        date: "2026-03-27",
        scores: { energy: 7, mood: 8 },
        created_at: "2026-03-27T10:00:00Z",
      };

      server.use(
        http.put("/api/v1/observer-polls/poll-1/respond", async ({ request }) => {
          capturedBody = await request.json();
          return HttpResponse.json(responseView);
        }),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      const result = await observerPollsApi.respond("poll-1", {
        date: "2026-03-27",
        scores: { energy: 7, mood: 8 },
      });

      expect(capturedBody).toEqual({
        date: "2026-03-27",
        scores: { energy: 7, mood: 8 },
      });
      expect(result).toEqual(responseView);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.put(
          "/api/v1/observer-polls/poll-1/respond",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(
        observerPollsApi.respond("poll-1", { date: "2026-03-27", scores: {} }),
      ).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.put(
          "/api/v1/observer-polls/poll-1/respond",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(
        observerPollsApi.respond("poll-1", { date: "2026-03-27", scores: {} }),
      ).rejects.toMatchObject({ name: "ApiError", status: 500 });
    });
  });

  describe("myResponses", () => {
    it("GETs /api/v1/observer-polls/:id/my-responses", async () => {
      const data = {
        responses: [
          { id: "r1", date: "2026-03-27", scores: { energy: 7 }, created_at: "2026-03-27T10:00:00Z" },
        ],
      };

      server.use(
        http.get("/api/v1/observer-polls/poll-1/my-responses", () =>
          HttpResponse.json(data),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      const result = await observerPollsApi.myResponses("poll-1");

      expect(result).toEqual(data);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.get(
          "/api/v1/observer-polls/poll-1/my-responses",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.myResponses("poll-1")).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.get(
          "/api/v1/observer-polls/poll-1/my-responses",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.myResponses("poll-1")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("deleteResponse", () => {
    it("DELETEs /api/v1/observer-polls/responses/:id", async () => {
      let called = false;

      server.use(
        http.delete("/api/v1/observer-polls/responses/resp-1", () => {
          called = true;
          return HttpResponse.json(null);
        }),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await observerPollsApi.deleteResponse("resp-1");

      expect(called).toBe(true);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.delete(
          "/api/v1/observer-polls/responses/resp-1",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.deleteResponse("resp-1")).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.delete(
          "/api/v1/observer-polls/responses/resp-1",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.deleteResponse("resp-1")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("exportResponses", () => {
    it("GETs /api/v1/observer-polls/export", async () => {
      const data = {
        responses: [
          {
            poll_name: "Daily check",
            date: "2026-03-27",
            scores: { energy: 7 },
            created_at: "2026-03-27T10:00:00Z",
          },
        ],
      };

      server.use(
        http.get("/api/v1/observer-polls/export", () => HttpResponse.json(data)),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      const result = await observerPollsApi.exportResponses();

      expect(result).toEqual(data);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.get(
          "/api/v1/observer-polls/export",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.exportResponses()).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.get(
          "/api/v1/observer-polls/export",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { observerPollsApi } = await import("../../src/api/observer-polls");
      await expect(observerPollsApi.exportResponses()).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });
});
