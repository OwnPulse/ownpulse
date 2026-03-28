// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { interventionsApi } from "../../src/api/interventions";
import { useAuthStore } from "../../src/store/auth";

const interventionsList = [
  {
    id: "iv-1",
    user_id: "user-1",
    substance: "Caffeine",
    dose: 200,
    unit: "mg",
    route: "oral",
    administered_at: "2026-03-02T08:00:00Z",
    fasted: false,
    created_at: "2026-03-02T08:00:00Z",
  },
  {
    id: "iv-2",
    user_id: "user-1",
    substance: "Magnesium",
    dose: 400,
    unit: "mg",
    route: "oral",
    administered_at: "2026-03-03T20:00:00Z",
    fasted: true,
    notes: "before bed",
    created_at: "2026-03-03T20:00:00Z",
  },
];

const server = setupServer(
  http.get("/api/v1/interventions", () => {
    return HttpResponse.json(interventionsList);
  }),
  http.get("/api/v1/interventions/:id", () => {
    return HttpResponse.json(interventionsList[0]);
  }),
  http.post("/api/v1/interventions", () => {
    return HttpResponse.json(interventionsList[0], { status: 201 });
  }),
  http.delete("/api/v1/interventions/:id", () => {
    return new HttpResponse(null, { status: 204 });
  }),
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

describe("interventionsApi", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  describe("list", () => {
    it("fetches interventions without params", async () => {
      const result = await interventionsApi.list();
      expect(result).toHaveLength(2);
      expect(result[0].substance).toBe("Caffeine");
    });

    it("fetches interventions with date filter params", async () => {
      server.use(
        http.get("/api/v1/interventions", ({ request }) => {
          const url = new URL(request.url);
          expect(url.searchParams.get("start")).toBe("2026-03-01T00:00:00Z");
          expect(url.searchParams.get("end")).toBe("2026-03-07T23:59:59Z");
          return HttpResponse.json([interventionsList[0]]);
        }),
      );
      const result = await interventionsApi.list({
        start: "2026-03-01T00:00:00Z",
        end: "2026-03-07T23:59:59Z",
      });
      expect(result).toHaveLength(1);
    });

    it("handles 401 error", async () => {
      server.use(
        http.get("/api/v1/interventions", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(interventionsApi.list()).rejects.toThrow("Unauthorized");
    });

    it("handles 500 error", async () => {
      server.use(
        http.get("/api/v1/interventions", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(interventionsApi.list()).rejects.toThrow("Internal Server Error");
    });

    it("handles 403 error", async () => {
      server.use(
        http.get("/api/v1/interventions", () => {
          return new HttpResponse("Forbidden", { status: 403 });
        }),
      );
      await expect(interventionsApi.list()).rejects.toThrow("Forbidden");
    });
  });

  describe("get", () => {
    it("fetches a single intervention", async () => {
      const result = await interventionsApi.get("iv-1");
      expect(result.id).toBe("iv-1");
      expect(result.substance).toBe("Caffeine");
    });

    it("handles 500 error", async () => {
      server.use(
        http.get("/api/v1/interventions/:id", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(interventionsApi.get("iv-1")).rejects.toThrow("Internal Server Error");
    });
  });

  describe("create", () => {
    it("creates an intervention", async () => {
      const result = await interventionsApi.create({
        substance: "Caffeine",
        dose: 200,
        unit: "mg",
        route: "oral",
        administered_at: "2026-03-02T08:00:00Z",
        fasted: false,
      });
      expect(result.id).toBe("iv-1");
    });

    it("handles 401 error", async () => {
      server.use(
        http.post("/api/v1/interventions", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(
        interventionsApi.create({
          substance: "Test",
          dose: 100,
          unit: "mg",
          route: "oral",
          administered_at: "2026-03-02T08:00:00Z",
          fasted: false,
        }),
      ).rejects.toThrow("Unauthorized");
    });
  });

  describe("delete", () => {
    it("deletes an intervention", async () => {
      await expect(interventionsApi.delete("iv-1")).resolves.not.toThrow();
    });

    it("handles 500 error", async () => {
      server.use(
        http.delete("/api/v1/interventions/:id", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(interventionsApi.delete("iv-1")).rejects.toThrow("Internal Server Error");
    });
  });
});
