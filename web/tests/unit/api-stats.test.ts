// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { HttpResponse, http } from "msw";
import { setupServer } from "msw/node";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "vitest";
import { statsApi } from "../../src/api/stats";
import { useAuthStore } from "../../src/store/auth";

const beforeAfterResponse = {
  intervention_substance: "Magnesium",
  first_dose: "2026-01-15T08:00:00Z",
  last_dose: null,
  metric: { source: "checkins", field: "energy" },
  before: {
    mean: 5.2,
    std_dev: 1.1,
    n: 30,
    points: [
      { t: "2026-01-01T00:00:00Z", v: 5 },
      { t: "2026-01-02T00:00:00Z", v: 6 },
    ],
  },
  after: {
    mean: 6.8,
    std_dev: 0.9,
    n: 30,
    points: [
      { t: "2026-01-16T00:00:00Z", v: 7 },
      { t: "2026-01-17T00:00:00Z", v: 6 },
    ],
  },
  change_pct: 30.8,
  p_value: 0.003,
  significant: true,
  test_used: "welch_t",
};

const correlateResponse = {
  metric_a: { source: "checkins", field: "energy" },
  metric_b: { source: "checkins", field: "mood" },
  r: 0.72,
  p_value: 0.001,
  n: 60,
  significant: true,
  method: "pearson",
  interpretation: "Strong positive correlation",
  scatter: [
    { a: 5, b: 6, t: "2026-01-01T00:00:00Z" },
    { a: 7, b: 8, t: "2026-01-02T00:00:00Z" },
  ],
};

const lagCorrelateResponse = {
  metric_a: { source: "checkins", field: "energy" },
  metric_b: { source: "checkins", field: "mood" },
  lags: [
    { lag: 0, r: 0.5, p_value: 0.01, n: 60 },
    { lag: 1, r: 0.72, p_value: 0.001, n: 59 },
    { lag: 2, r: 0.4, p_value: 0.05, n: 58 },
  ],
  best_lag: { lag: 1, r: 0.72, p_value: 0.001, n: 59 },
  method: "pearson",
};

const server = setupServer(
  http.post("/api/v1/stats/before-after", () => {
    return HttpResponse.json(beforeAfterResponse);
  }),
  http.post("/api/v1/stats/correlate", () => {
    return HttpResponse.json(correlateResponse);
  }),
  http.post("/api/v1/stats/lag-correlate", () => {
    return HttpResponse.json(lagCorrelateResponse);
  }),
);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

describe("statsApi", () => {
  beforeEach(() => {
    useAuthStore.getState().login("test-jwt-token");
  });

  afterEach(() => {
    useAuthStore.getState().logout();
  });

  describe("beforeAfter", () => {
    it("posts before-after request and returns response", async () => {
      const result = await statsApi.beforeAfter({
        intervention_substance: "Magnesium",
        metric: { source: "checkins", field: "energy" },
        before_days: 30,
        after_days: 30,
        resolution: "daily",
      });
      expect(result.intervention_substance).toBe("Magnesium");
      expect(result.before.mean).toBe(5.2);
      expect(result.after.mean).toBe(6.8);
      expect(result.significant).toBe(true);
    });

    it("handles 401 error", async () => {
      server.use(
        http.post("/api/v1/stats/before-after", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(
        statsApi.beforeAfter({
          intervention_substance: "Magnesium",
          metric: { source: "checkins", field: "energy" },
          before_days: 30,
          after_days: 30,
          resolution: "daily",
        }),
      ).rejects.toThrow("Unauthorized");
    });

    it("handles 500 error", async () => {
      server.use(
        http.post("/api/v1/stats/before-after", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(
        statsApi.beforeAfter({
          intervention_substance: "Magnesium",
          metric: { source: "checkins", field: "energy" },
          before_days: 30,
          after_days: 30,
          resolution: "daily",
        }),
      ).rejects.toThrow("Internal Server Error");
    });

    it("handles 403 error", async () => {
      server.use(
        http.post("/api/v1/stats/before-after", () => {
          return new HttpResponse("Forbidden", { status: 403 });
        }),
      );
      await expect(
        statsApi.beforeAfter({
          intervention_substance: "Magnesium",
          metric: { source: "checkins", field: "energy" },
          before_days: 30,
          after_days: 30,
          resolution: "daily",
        }),
      ).rejects.toThrow("Forbidden");
    });
  });

  describe("correlate", () => {
    it("posts correlate request and returns response", async () => {
      const result = await statsApi.correlate({
        metric_a: { source: "checkins", field: "energy" },
        metric_b: { source: "checkins", field: "mood" },
        start: "2026-01-01T00:00:00Z",
        end: "2026-03-01T23:59:59Z",
        resolution: "daily",
        method: "pearson",
      });
      expect(result.r).toBe(0.72);
      expect(result.significant).toBe(true);
      expect(result.scatter).toHaveLength(2);
    });

    it("handles 401 error", async () => {
      server.use(
        http.post("/api/v1/stats/correlate", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(
        statsApi.correlate({
          metric_a: { source: "checkins", field: "energy" },
          metric_b: { source: "checkins", field: "mood" },
          start: "2026-01-01T00:00:00Z",
          end: "2026-03-01T23:59:59Z",
          resolution: "daily",
          method: "pearson",
        }),
      ).rejects.toThrow("Unauthorized");
    });

    it("handles 500 error", async () => {
      server.use(
        http.post("/api/v1/stats/correlate", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(
        statsApi.correlate({
          metric_a: { source: "checkins", field: "energy" },
          metric_b: { source: "checkins", field: "mood" },
          start: "2026-01-01T00:00:00Z",
          end: "2026-03-01T23:59:59Z",
          resolution: "daily",
          method: "pearson",
        }),
      ).rejects.toThrow("Internal Server Error");
    });
  });

  describe("lagCorrelate", () => {
    it("posts lag-correlate request and returns response", async () => {
      const result = await statsApi.lagCorrelate({
        metric_a: { source: "checkins", field: "energy" },
        metric_b: { source: "checkins", field: "mood" },
        start: "2026-01-01T00:00:00Z",
        end: "2026-03-01T23:59:59Z",
        resolution: "daily",
        max_lag_days: 7,
        method: "pearson",
      });
      expect(result.lags).toHaveLength(3);
      expect(result.best_lag.lag).toBe(1);
      expect(result.best_lag.r).toBe(0.72);
      expect(result.method).toBe("pearson");
    });

    it("handles 401 error", async () => {
      server.use(
        http.post("/api/v1/stats/lag-correlate", () => {
          return new HttpResponse("Unauthorized", { status: 401 });
        }),
      );
      await expect(
        statsApi.lagCorrelate({
          metric_a: { source: "checkins", field: "energy" },
          metric_b: { source: "checkins", field: "mood" },
          start: "2026-01-01T00:00:00Z",
          end: "2026-03-01T23:59:59Z",
          resolution: "daily",
          max_lag_days: 7,
          method: "pearson",
        }),
      ).rejects.toThrow("Unauthorized");
    });

    it("handles 500 error", async () => {
      server.use(
        http.post("/api/v1/stats/lag-correlate", () => {
          return new HttpResponse("Internal Server Error", { status: 500 });
        }),
      );
      await expect(
        statsApi.lagCorrelate({
          metric_a: { source: "checkins", field: "energy" },
          metric_b: { source: "checkins", field: "mood" },
          start: "2026-01-01T00:00:00Z",
          end: "2026-03-01T23:59:59Z",
          resolution: "daily",
          max_lag_days: 7,
          method: "pearson",
        }),
      ).rejects.toThrow("Internal Server Error");
    });

    it("handles 403 error", async () => {
      server.use(
        http.post("/api/v1/stats/lag-correlate", () => {
          return new HttpResponse("Forbidden", { status: 403 });
        }),
      );
      await expect(
        statsApi.lagCorrelate({
          metric_a: { source: "checkins", field: "energy" },
          metric_b: { source: "checkins", field: "mood" },
          start: "2026-01-01T00:00:00Z",
          end: "2026-03-01T23:59:59Z",
          resolution: "daily",
          max_lag_days: 7,
          method: "pearson",
        }),
      ).rejects.toThrow("Forbidden");
    });
  });
});
