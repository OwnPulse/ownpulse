// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { http, HttpResponse } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { exploreApi } from "../../src/api/explore";
import { useAuthStore } from "../../src/store/auth";

const metricsResponse = {
  sources: [
    {
      source: "checkins",
      label: "Check-ins",
      metrics: [
        { field: "energy", label: "Energy", unit: "score" },
        { field: "mood", label: "Mood", unit: "score" },
      ],
    },
  ],
};

const batchSeriesResponse = {
  series: [
    {
      source: "checkins",
      field: "energy",
      unit: "score",
      points: [
        { t: "2026-03-01T00:00:00Z", v: 7, n: 1 },
        { t: "2026-03-02T00:00:00Z", v: 6, n: 1 },
      ],
    },
  ],
};

const savedChart = {
  id: "chart-1",
  name: "My Chart",
  config: {
    version: 1,
    metrics: [{ source: "checkins", field: "energy" }],
    range: { preset: "30d" },
    resolution: "daily",
  },
  created_at: "2026-03-01T00:00:00Z",
  updated_at: "2026-03-01T00:00:00Z",
};

const server = setupServer(
  http.get("/api/v1/explore/metrics", () => {
    return HttpResponse.json(metricsResponse);
  }),
  http.post("/api/v1/explore/series", () => {
    return HttpResponse.json(batchSeriesResponse);
  }),
  http.get("/api/v1/explore/charts", () => {
    return HttpResponse.json([savedChart]);
  }),
  http.post("/api/v1/explore/charts", () => {
    return HttpResponse.json(savedChart, { status: 201 });
  }),
  http.get("/api/v1/explore/charts/:id", () => {
    return HttpResponse.json(savedChart);
  }),
  http.put("/api/v1/explore/charts/:id", () => {
    return HttpResponse.json(savedChart);
  }),
  http.delete("/api/v1/explore/charts/:id", () => {
    return new HttpResponse(null, { status: 204 });
  }),
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

describe("exploreApi", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  describe("getMetrics", () => {
    it("fetches metrics successfully", async () => {
      const result = await exploreApi.getMetrics();
      expect(result.sources).toHaveLength(1);
      expect(result.sources[0].source).toBe("checkins");
      expect(result.sources[0].metrics).toHaveLength(2);
    });

    it("handles 401 error", async () => {
      server.use(
        http.get("/api/v1/explore/metrics", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(exploreApi.getMetrics()).rejects.toThrow("Unauthorized");
    });

    it("handles 500 error", async () => {
      server.use(
        http.get("/api/v1/explore/metrics", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(exploreApi.getMetrics()).rejects.toThrow("Internal Server Error");
    });
  });

  describe("batchSeries", () => {
    it("fetches batch series successfully", async () => {
      const result = await exploreApi.batchSeries({
        metrics: [{ source: "checkins", field: "energy" }],
        start: "2026-03-01",
        end: "2026-03-07",
        resolution: "daily",
      });
      expect(result.series).toHaveLength(1);
      expect(result.series[0].points).toHaveLength(2);
    });

    it("handles 403 error", async () => {
      server.use(
        http.post("/api/v1/explore/series", () => {
          return new HttpResponse("Forbidden", { status: 403 });
        }),
      );
      await expect(
        exploreApi.batchSeries({
          metrics: [{ source: "checkins", field: "energy" }],
          start: "2026-03-01",
          end: "2026-03-07",
          resolution: "daily",
        }),
      ).rejects.toThrow("Forbidden");
    });
  });

  describe("listCharts", () => {
    it("fetches saved charts", async () => {
      const result = await exploreApi.listCharts();
      expect(result).toHaveLength(1);
      expect(result[0].name).toBe("My Chart");
    });

    it("handles 500 error", async () => {
      server.use(
        http.get("/api/v1/explore/charts", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(exploreApi.listCharts()).rejects.toThrow("Internal Server Error");
    });
  });

  describe("createChart", () => {
    it("creates a chart successfully", async () => {
      const result = await exploreApi.createChart({
        name: "My Chart",
        config: {
          version: 1,
          metrics: [{ source: "checkins", field: "energy" }],
          range: { preset: "30d" },
          resolution: "daily",
        },
      });
      expect(result.id).toBe("chart-1");
      expect(result.name).toBe("My Chart");
    });

    it("handles 401 error", async () => {
      server.use(
        http.post("/api/v1/explore/charts", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(
        exploreApi.createChart({
          name: "Test",
          config: {
            version: 1,
            metrics: [],
            range: { preset: "30d" },
            resolution: "daily",
          },
        }),
      ).rejects.toThrow("Unauthorized");
    });
  });

  describe("getChart", () => {
    it("fetches a chart by ID", async () => {
      const result = await exploreApi.getChart("chart-1");
      expect(result.id).toBe("chart-1");
    });

    it("handles 500 error", async () => {
      server.use(
        http.get("/api/v1/explore/charts/:id", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(exploreApi.getChart("chart-1")).rejects.toThrow("Internal Server Error");
    });
  });

  describe("updateChart", () => {
    it("updates a chart", async () => {
      const result = await exploreApi.updateChart("chart-1", { name: "Updated" });
      expect(result.id).toBe("chart-1");
    });

    it("handles 500 error", async () => {
      server.use(
        http.put("/api/v1/explore/charts/:id", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(exploreApi.updateChart("chart-1", { name: "X" })).rejects.toThrow(
        "Internal Server Error",
      );
    });
  });

  describe("deleteChart", () => {
    it("deletes a chart", async () => {
      await expect(exploreApi.deleteChart("chart-1")).resolves.not.toThrow();
    });

    it("handles 500 error", async () => {
      server.use(
        http.delete("/api/v1/explore/charts/:id", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(exploreApi.deleteChart("chart-1")).rejects.toThrow("Internal Server Error");
    });
  });

  describe("getSeries", () => {
    it("fetches a single series with query params", async () => {
      server.use(
        http.get("/api/v1/explore/series", ({ request }) => {
          const url = new URL(request.url);
          expect(url.searchParams.get("source")).toBe("checkins");
          expect(url.searchParams.get("field")).toBe("energy");
          return HttpResponse.json({
            source: "checkins",
            field: "energy",
            unit: "score",
            points: [{ t: "2026-03-01T00:00:00Z", v: 7, n: 1 }],
          });
        }),
      );

      const result = await exploreApi.getSeries({
        source: "checkins",
        field: "energy",
        start: "2026-03-01",
        end: "2026-03-07",
        resolution: "daily",
      });
      expect(result.source).toBe("checkins");
      expect(result.points).toHaveLength(1);
    });
  });
});
