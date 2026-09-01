// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { useAuthStore } from "../../src/store/auth";
import { refresh401Handler } from "./support/msw-auth-refresh";

const TOKEN = "test-jwt";
const server = setupServer(refresh401Handler);

beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

const doseRow = {
  id: "dose-1",
  protocol_line_id: "line-1",
  day_number: 3,
  status: "completed",
  intervention_id: "iv-1",
  logged_at: "2026-03-28T08:00:00Z",
  run_id: "run-1",
  skip_reason: null,
};

describe("protocolsApi dose/adherence methods", () => {
  beforeEach(() => {
    useAuthStore.setState({ token: TOKEN, isAuthenticated: true });
    vi.useRealTimers();
  });

  describe("logRunDose", () => {
    it("POSTs /api/v1/protocols/runs/:runId/doses/log and always sends tz_offset_minutes", async () => {
      // Fixed offset so the assertion is deterministic regardless of the
      // machine's local timezone.
      vi.spyOn(Date.prototype, "getTimezoneOffset").mockReturnValue(420); // UTC-7

      let capturedBody: unknown;
      server.use(
        http.post("/api/v1/protocols/runs/:runId/doses/log", async ({ params, request }) => {
          expect(params.runId).toBe("run-1");
          capturedBody = await request.json();
          return HttpResponse.json(doseRow);
        }),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      const result = await protocolsApi.logRunDose("run-1", {
        protocol_line_id: "line-1",
        day_number: 3,
      });

      expect(result).toEqual(doseRow);
      expect(capturedBody).toEqual({
        protocol_line_id: "line-1",
        day_number: 3,
        tz_offset_minutes: -420,
      });

      vi.restoreAllMocks();
    });

    it("respects an explicitly-passed tz_offset_minutes instead of the browser's", async () => {
      let capturedBody: unknown;
      server.use(
        http.post("/api/v1/protocols/runs/:runId/doses/log", async ({ request }) => {
          capturedBody = await request.json();
          return HttpResponse.json(doseRow);
        }),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      await protocolsApi.logRunDose("run-1", {
        protocol_line_id: "line-1",
        day_number: 3,
        tz_offset_minutes: -60,
        administered_at: "2026-03-28T08:00:00Z",
        notes: "with food",
      });

      expect(capturedBody).toEqual({
        protocol_line_id: "line-1",
        day_number: 3,
        tz_offset_minutes: -60,
        administered_at: "2026-03-28T08:00:00Z",
        notes: "with food",
      });
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/runs/:runId/doses/log",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(
        protocolsApi.logRunDose("run-1", { protocol_line_id: "line-1", day_number: 3 }),
      ).rejects.toThrow("Unauthorized");
      expect(useAuthStore.getState().isAuthenticated).toBe(false);
    });

    it("throws ApiError on 403", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/runs/:runId/doses/log",
          () => new HttpResponse("Forbidden", { status: 403 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(
        protocolsApi.logRunDose("run-1", { protocol_line_id: "line-1", day_number: 3 }),
      ).rejects.toMatchObject({ name: "ApiError", status: 403 });
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/runs/:runId/doses/log",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(
        protocolsApi.logRunDose("run-1", { protocol_line_id: "line-1", day_number: 3 }),
      ).rejects.toMatchObject({ name: "ApiError", status: 500 });
    });
  });

  describe("skipRunDose", () => {
    it("POSTs /api/v1/protocols/runs/:runId/doses/skip with the optional skip_reason and returns nothing (204)", async () => {
      let capturedBody: unknown;
      server.use(
        http.post("/api/v1/protocols/runs/:runId/doses/skip", async ({ params, request }) => {
          expect(params.runId).toBe("run-1");
          capturedBody = await request.json();
          // `skip_dose_on_run` in the backend returns 204 No Content — no
          // dose row, unlike log.
          return new HttpResponse(null, { status: 204 });
        }),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      const result = await protocolsApi.skipRunDose("run-1", {
        protocol_line_id: "line-1",
        day_number: 3,
        skip_reason: "traveling",
      });

      expect(result).toBeUndefined();
      expect(capturedBody).toEqual({
        protocol_line_id: "line-1",
        day_number: 3,
        skip_reason: "traveling",
      });
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/runs/:runId/doses/skip",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(
        protocolsApi.skipRunDose("run-1", { protocol_line_id: "line-1", day_number: 3 }),
      ).rejects.toThrow("Unauthorized");
    });

    it("throws ApiError on 403", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/runs/:runId/doses/skip",
          () => new HttpResponse("Forbidden", { status: 403 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(
        protocolsApi.skipRunDose("run-1", { protocol_line_id: "line-1", day_number: 3 }),
      ).rejects.toMatchObject({ name: "ApiError", status: 403 });
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.post(
          "/api/v1/protocols/runs/:runId/doses/skip",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(
        protocolsApi.skipRunDose("run-1", { protocol_line_id: "line-1", day_number: 3 }),
      ).rejects.toMatchObject({ name: "ApiError", status: 500 });
    });
  });

  describe("deleteRunDose", () => {
    it("DELETEs /api/v1/protocols/runs/:runId/doses/:doseId", async () => {
      let called = false;
      server.use(
        http.delete("/api/v1/protocols/runs/:runId/doses/:doseId", ({ params }) => {
          called = true;
          expect(params.runId).toBe("run-1");
          expect(params.doseId).toBe("dose-1");
          return new HttpResponse(null, { status: 204 });
        }),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      await protocolsApi.deleteRunDose("run-1", "dose-1");
      expect(called).toBe(true);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.delete(
          "/api/v1/protocols/runs/:runId/doses/:doseId",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.deleteRunDose("run-1", "dose-1")).rejects.toThrow("Unauthorized");
    });

    it("throws ApiError on 403", async () => {
      server.use(
        http.delete(
          "/api/v1/protocols/runs/:runId/doses/:doseId",
          () => new HttpResponse("Forbidden", { status: 403 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.deleteRunDose("run-1", "dose-1")).rejects.toMatchObject({
        name: "ApiError",
        status: 403,
      });
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.delete(
          "/api/v1/protocols/runs/:runId/doses/:doseId",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.deleteRunDose("run-1", "dose-1")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("runDoses", () => {
    const runDoseItem = {
      day_number: 3,
      date: "2026-03-31",
      protocol_line_id: "line-1",
      substance: "BPC-157",
      dose: 250,
      unit: "mcg",
      route: "SubQ",
      time_of_day: "AM",
      status: "missed",
      dose_id: null,
      intervention_id: null,
      skip_reason: null,
      logged_at: null,
    };

    it("GETs /api/v1/protocols/runs/:runId/doses with from_day/to_day query params", async () => {
      let capturedUrl: string | undefined;
      server.use(
        http.get("/api/v1/protocols/runs/:runId/doses", ({ request }) => {
          capturedUrl = request.url;
          return HttpResponse.json([runDoseItem]);
        }),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      const result = await protocolsApi.runDoses("run-1", { fromDay: 0, toDay: 27 });

      expect(result).toEqual([runDoseItem]);
      expect(capturedUrl).toContain("from_day=0");
      expect(capturedUrl).toContain("to_day=27");
    });

    it("omits query params entirely when no range is given", async () => {
      let capturedUrl: string | undefined;
      server.use(
        http.get("/api/v1/protocols/runs/:runId/doses", ({ request }) => {
          capturedUrl = request.url;
          return HttpResponse.json([]);
        }),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      await protocolsApi.runDoses("run-1");

      expect(capturedUrl).toBe("http://localhost:3000/api/v1/protocols/runs/run-1/doses");
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.get(
          "/api/v1/protocols/runs/:runId/doses",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.runDoses("run-1")).rejects.toThrow("Unauthorized");
    });

    it("throws ApiError on 403", async () => {
      server.use(
        http.get(
          "/api/v1/protocols/runs/:runId/doses",
          () => new HttpResponse("Forbidden", { status: 403 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.runDoses("run-1")).rejects.toMatchObject({
        name: "ApiError",
        status: 403,
      });
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.get(
          "/api/v1/protocols/runs/:runId/doses",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.runDoses("run-1")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("missedDoses", () => {
    const missedItem = {
      protocol_id: "p1",
      protocol_name: "BPC Stack",
      run_id: "run-1",
      protocol_line_id: "line-1",
      substance: "BPC-157",
      dose: 250,
      unit: "mcg",
      route: "SubQ",
      time_of_day: "AM",
      day_number: 2,
      date: "2026-03-30",
      status: "missed",
    };

    it("GETs /api/v1/protocols/runs/missed-doses", async () => {
      server.use(
        http.get("/api/v1/protocols/runs/missed-doses", () => HttpResponse.json([missedItem])),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      const result = await protocolsApi.missedDoses();

      expect(result).toEqual([missedItem]);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.get(
          "/api/v1/protocols/runs/missed-doses",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.missedDoses()).rejects.toThrow("Unauthorized");
    });

    it("throws ApiError on 403", async () => {
      server.use(
        http.get(
          "/api/v1/protocols/runs/missed-doses",
          () => new HttpResponse("Forbidden", { status: 403 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.missedDoses()).rejects.toMatchObject({
        name: "ApiError",
        status: 403,
      });
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.get(
          "/api/v1/protocols/runs/missed-doses",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.missedDoses()).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });

  describe("runAdherence", () => {
    const adherenceResponse = {
      run_id: "run-1",
      scheduled_so_far: 10,
      completed: 8,
      skipped: 1,
      missed: 1,
      adherence_pct: 88.9,
      lines: [],
    };

    it("GETs /api/v1/protocols/runs/:runId/adherence", async () => {
      server.use(
        http.get("/api/v1/protocols/runs/:runId/adherence", ({ params }) => {
          expect(params.runId).toBe("run-1");
          return HttpResponse.json(adherenceResponse);
        }),
      );

      const { protocolsApi } = await import("../../src/api/protocols");
      const result = await protocolsApi.runAdherence("run-1");

      expect(result).toEqual(adherenceResponse);
    });

    it("throws on 401 and triggers logout", async () => {
      server.use(
        http.get(
          "/api/v1/protocols/runs/:runId/adherence",
          () => new HttpResponse("Unauthorized", { status: 401 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.runAdherence("run-1")).rejects.toThrow("Unauthorized");
    });

    it("throws ApiError on 403", async () => {
      server.use(
        http.get(
          "/api/v1/protocols/runs/:runId/adherence",
          () => new HttpResponse("Forbidden", { status: 403 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.runAdherence("run-1")).rejects.toMatchObject({
        name: "ApiError",
        status: 403,
      });
    });

    it("throws ApiError on 500", async () => {
      server.use(
        http.get(
          "/api/v1/protocols/runs/:runId/adherence",
          () => new HttpResponse("Server error", { status: 500 }),
        ),
      );
      const { protocolsApi } = await import("../../src/api/protocols");
      await expect(protocolsApi.runAdherence("run-1")).rejects.toMatchObject({
        name: "ApiError",
        status: 500,
      });
    });
  });
});
