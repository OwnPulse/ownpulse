// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { protocolsApi } from "../../src/api/protocols";
import { useAuthStore } from "../../src/store/auth";

const todaysDosesList = [
  {
    protocol_id: "p1",
    protocol_name: "BPC Stack",
    protocol_line_id: "pl-1",
    run_id: "run-1",
    substance: "BPC-157",
    dose: 250,
    unit: "mcg",
    route: "SubQ",
    time_of_day: "08:00",
    day_number: 3,
    status: "pending" as const,
    dose_id: null,
  },
  {
    protocol_id: "p1",
    protocol_name: "BPC Stack",
    protocol_line_id: "pl-2",
    run_id: "run-1",
    substance: "TB-500",
    dose: 2,
    unit: "mg",
    route: "SubQ",
    time_of_day: "08:00",
    day_number: 3,
    status: "completed" as const,
    dose_id: "dose-1",
  },
];

const loggedDose = {
  id: "dose-new",
  protocol_line_id: "pl-1",
  day_number: 3,
  status: "completed" as const,
  intervention_id: "iv-1",
  logged_at: "2026-03-28T08:00:00Z",
  created_at: "2026-03-28T08:00:00Z",
};

const skippedDose = {
  id: "dose-skip",
  protocol_line_id: "pl-1",
  day_number: 3,
  status: "skipped" as const,
  intervention_id: null,
  logged_at: "2026-03-28T08:00:00Z",
  created_at: "2026-03-28T08:00:00Z",
};

const server = setupServer(
  http.get("/api/v1/protocols/todays-doses", () => {
    return HttpResponse.json(todaysDosesList);
  }),
  http.post("/api/v1/protocols/runs/:runId/doses/log", () => {
    return HttpResponse.json(loggedDose);
  }),
  http.post("/api/v1/protocols/runs/:runId/doses/skip", () => {
    return HttpResponse.json(skippedDose);
  }),
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

describe("protocolsApi - todays doses", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  describe("todaysDoses", () => {
    it("fetches todays doses", async () => {
      const result = await protocolsApi.todaysDoses();
      expect(result).toHaveLength(2);
      expect(result[0].substance).toBe("BPC-157");
      expect(result[0].run_id).toBe("run-1");
      expect(result[0].status).toBe("pending");
      expect(result[1].status).toBe("completed");
    });

    it("handles 401 error", async () => {
      server.use(
        http.get("/api/v1/protocols/todays-doses", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(protocolsApi.todaysDoses()).rejects.toThrow("Unauthorized");
    });

    it("handles 403 error", async () => {
      server.use(
        http.get("/api/v1/protocols/todays-doses", () => {
          return new HttpResponse("Forbidden", { status: 403 });
        }),
      );
      await expect(protocolsApi.todaysDoses()).rejects.toThrow("Forbidden");
    });

    it("handles 500 error", async () => {
      server.use(
        http.get("/api/v1/protocols/todays-doses", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(protocolsApi.todaysDoses()).rejects.toThrow("Internal Server Error");
    });
  });

  describe("logRunDose", () => {
    it("logs a dose on a run", async () => {
      const result = await protocolsApi.logRunDose("run-1", {
        protocol_line_id: "pl-1",
        day_number: 3,
      });
      expect(result.id).toBe("dose-new");
      expect(result.status).toBe("completed");
    });

    it("handles 401 error", async () => {
      server.use(
        http.post("/api/v1/protocols/runs/:runId/doses/log", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(
        protocolsApi.logRunDose("run-1", { protocol_line_id: "pl-1", day_number: 3 }),
      ).rejects.toThrow("Unauthorized");
    });

    it("handles 500 error", async () => {
      server.use(
        http.post("/api/v1/protocols/runs/:runId/doses/log", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(
        protocolsApi.logRunDose("run-1", { protocol_line_id: "pl-1", day_number: 3 }),
      ).rejects.toThrow("Internal Server Error");
    });
  });

  describe("skipRunDose", () => {
    it("skips a dose on a run", async () => {
      const result = await protocolsApi.skipRunDose("run-1", {
        protocol_line_id: "pl-1",
        day_number: 3,
      });
      expect(result.id).toBe("dose-skip");
      expect(result.status).toBe("skipped");
    });

    it("handles 401 error", async () => {
      server.use(
        http.post("/api/v1/protocols/runs/:runId/doses/skip", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(
        protocolsApi.skipRunDose("run-1", { protocol_line_id: "pl-1", day_number: 3 }),
      ).rejects.toThrow("Unauthorized");
    });

    it("handles 500 error", async () => {
      server.use(
        http.post("/api/v1/protocols/runs/:runId/doses/skip", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(
        protocolsApi.skipRunDose("run-1", { protocol_line_id: "pl-1", day_number: 3 }),
      ).rejects.toThrow("Internal Server Error");
    });
  });
});
