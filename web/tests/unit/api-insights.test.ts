// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { insightsApi } from "../../src/api/insights";
import { useAuthStore } from "../../src/store/auth";

const insightsResponse = [
  {
    id: "i1",
    insight_type: "trend",
    headline: "Energy trending up 15%",
    detail: "Average went from 5.2 to 6.0",
    metadata: { explore_params: { source: "checkins", field: "energy", preset: "30d" } },
    created_at: "2026-03-28T06:00:00Z",
  },
  {
    id: "i2",
    insight_type: "streak",
    headline: "14-day check-in streak!",
    detail: null,
    metadata: {},
    created_at: "2026-03-28T06:00:00Z",
  },
];

const server = setupServer(
  http.get("/api/v1/insights", () => {
    return HttpResponse.json(insightsResponse);
  }),
  http.post("/api/v1/insights/:id/dismiss", () => {
    return new HttpResponse(null, { status: 204 });
  }),
  http.post("/api/v1/insights/generate", () => {
    return HttpResponse.json(insightsResponse);
  }),
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

describe("insightsApi", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  describe("list", () => {
    it("fetches insights successfully", async () => {
      const result = await insightsApi.list();
      expect(result).toHaveLength(2);
      expect(result[0].insight_type).toBe("trend");
      expect(result[0].headline).toBe("Energy trending up 15%");
      expect(result[1].insight_type).toBe("streak");
    });

    it("handles 401 error", async () => {
      server.use(
        http.get("/api/v1/insights", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(insightsApi.list()).rejects.toThrow("Unauthorized");
    });

    it("handles 403 error", async () => {
      server.use(
        http.get("/api/v1/insights", () => {
          return new HttpResponse("Forbidden", { status: 403 });
        }),
      );
      await expect(insightsApi.list()).rejects.toThrow("Forbidden");
    });

    it("handles 500 error", async () => {
      server.use(
        http.get("/api/v1/insights", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(insightsApi.list()).rejects.toThrow("Internal Server Error");
    });
  });

  describe("dismiss", () => {
    it("dismisses an insight successfully", async () => {
      await expect(insightsApi.dismiss("i1")).resolves.not.toThrow();
    });

    it("handles 401 error", async () => {
      server.use(
        http.post("/api/v1/insights/:id/dismiss", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(insightsApi.dismiss("i1")).rejects.toThrow("Unauthorized");
    });

    it("handles 500 error", async () => {
      server.use(
        http.post("/api/v1/insights/:id/dismiss", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(insightsApi.dismiss("i1")).rejects.toThrow("Internal Server Error");
    });
  });

  describe("generate", () => {
    it("generates insights successfully", async () => {
      const result = await insightsApi.generate();
      expect(result).toHaveLength(2);
    });

    it("handles 401 error", async () => {
      server.use(
        http.post("/api/v1/insights/generate", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(insightsApi.generate()).rejects.toThrow("Unauthorized");
    });

    it("handles 500 error", async () => {
      server.use(
        http.post("/api/v1/insights/generate", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(insightsApi.generate()).rejects.toThrow("Internal Server Error");
    });
  });
});
