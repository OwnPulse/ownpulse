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

describe("protocolsApi run methods", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: TOKEN, isAuthenticated: true });
  });

  describe("startRun", () => {
    it("POSTs /api/v1/protocols/:id/runs and returns a run", async () => {
      const runResponse = {
        id: "run-1",
        protocol_id: "proto-1",
        user_id: "user-1",
        start_date: "2026-03-28",
        status: "active",
        notify: false,
        notify_times: [],
        repeat_reminders: false,
        repeat_interval_minutes: 30,
        created_at: "2026-03-28T10:00:00Z",
      };

      let capturedBody: unknown;
      server.use(
        http.post("/api/v1/protocols/:id/runs", async ({ params, request }) => {
          expect(params.id).toBe("proto-1");
          capturedBody = await request.json();
          return HttpResponse.json(runResponse);
        }),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      const result = await protocolsApi.startRun("proto-1", {
        start_date: "2026-03-28",
        notify: false,
      });

      expect(result).toEqual(runResponse);
      expect(capturedBody).toEqual({ start_date: "2026-03-28", notify: false });
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/:id/runs",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.startRun("proto-1", {})).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 403", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/:id/runs",
          () => new HttpResponse("Forbidden", { status: 403 }),
        ),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.startRun("proto-1", {})).rejects.toMatchObject({
        name: "ApiError",
        status: 403,
      });
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/:id/runs",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.startRun("proto-1", {})).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("listRuns", () => {
    it("GETs /api/v1/protocols/:id/runs and returns runs", async () => {
      const runsResponse = [
        {
          id: "run-1",
          protocol_id: "proto-1",
          user_id: "user-1",
          start_date: "2026-03-28",
          status: "active",
          notify: false,
          notify_times: [],
          repeat_reminders: false,
          repeat_interval_minutes: 30,
          created_at: "2026-03-28T10:00:00Z",
        },
        {
          id: "run-2",
          protocol_id: "proto-1",
          user_id: "user-1",
          start_date: "2026-02-01",
          status: "completed",
          notify: true,
          notify_times: ["08:00"],
          repeat_reminders: false,
          repeat_interval_minutes: 30,
          created_at: "2026-02-01T10:00:00Z",
        },
      ];

      server.use(
        http.get("/api/v1/protocols/:id/runs", ({ params }) => {
          expect(params.id).toBe("proto-1");
          return HttpResponse.json(runsResponse);
        }),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      const result = await protocolsApi.listRuns("proto-1");

      expect(result).toEqual(runsResponse);
      expect(result).toHaveLength(2);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.get(
          "/api/v1/protocols/:id/runs",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.listRuns("proto-1")).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.get(
          "/api/v1/protocols/:id/runs",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.listRuns("proto-1")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("activeRuns", () => {
    it("GETs /api/v1/protocols/runs/active and returns active runs", async () => {
      const activeResponse = [
        {
          run: {
            id: "run-1",
            protocol_id: "proto-1",
            user_id: "user-1",
            start_date: "2026-03-28",
            status: "active",
            notify: false,
            notify_times: [],
            repeat_reminders: false,
            repeat_interval_minutes: 30,
            created_at: "2026-03-28T10:00:00Z",
          },
          protocol_name: "BPC Stack",
          doses_today: 3,
          doses_completed_today: 1,
          total_doses: 60,
          completed_doses: 20,
        },
      ];

      server.use(
        http.get("/api/v1/protocols/runs/active", () => {
          return HttpResponse.json(activeResponse);
        }),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      const result = await protocolsApi.activeRuns();

      expect(result).toEqual(activeResponse);
      expect(result[0].protocol_name).toBe("BPC Stack");
      expect(result[0].doses_today).toBe(3);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.get(
          "/api/v1/protocols/runs/active",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.activeRuns()).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.get(
          "/api/v1/protocols/runs/active",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.activeRuns()).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("updateRun", () => {
    it("PATCHes /api/v1/protocols/runs/:runId and returns updated run", async () => {
      const updatedRun = {
        id: "run-1",
        protocol_id: "proto-1",
        user_id: "user-1",
        start_date: "2026-03-28",
        status: "paused",
        notify: false,
        notify_times: [],
        repeat_reminders: false,
        repeat_interval_minutes: 30,
        created_at: "2026-03-28T10:00:00Z",
      };

      let capturedBody: unknown;
      server.use(
        http.patch("/api/v1/protocols/runs/:runId", async ({ params, request }) => {
          expect(params.runId).toBe("run-1");
          capturedBody = await request.json();
          return HttpResponse.json(updatedRun);
        }),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      const result = await protocolsApi.updateRun("run-1", { status: "paused" });

      expect(result).toEqual(updatedRun);
      expect(capturedBody).toEqual({ status: "paused" });
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.patch(
          "/api/v1/protocols/runs/:runId",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.updateRun("run-1", { status: "paused" })).rejects.toThrow(
        "Unauthorized",
      );
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 403", async () => {
      server.use(
        http.patch(
          "/api/v1/protocols/runs/:runId",
          () => new HttpResponse("Forbidden", { status: 403 }),
        ),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.updateRun("run-1", { status: "paused" })).rejects.toMatchObject({
        name: "ApiError",
        status: 403,
      });
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.patch(
          "/api/v1/protocols/runs/:runId",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.updateRun("run-1", { status: "paused" })).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });
});
