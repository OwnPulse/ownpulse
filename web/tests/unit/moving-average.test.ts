// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { describe, expect, it } from "vitest";
import { computeMovingAverage } from "../../src/components/explore/ExploreChart";

describe("computeMovingAverage", () => {
  it("returns empty array for empty input", () => {
    expect(computeMovingAverage([], 7)).toEqual([]);
  });

  it("returns the point itself for a single point", () => {
    const points = [{ t: "2026-03-01T00:00:00Z", v: 10 }];
    const result = computeMovingAverage(points, 7);
    expect(result).toEqual([{ t: "2026-03-01T00:00:00Z", v: 10 }]);
  });

  it("returns partial averages when array is shorter than window", () => {
    const points = [
      { t: "2026-03-01T00:00:00Z", v: 2 },
      { t: "2026-03-02T00:00:00Z", v: 4 },
      { t: "2026-03-03T00:00:00Z", v: 6 },
    ];
    const result = computeMovingAverage(points, 7);
    expect(result).toHaveLength(3);
    // First point: avg of [2] = 2
    expect(result[0].v).toBe(2);
    // Second point: avg of [2, 4] = 3
    expect(result[1].v).toBe(3);
    // Third point: avg of [2, 4, 6] = 4
    expect(result[2].v).toBe(4);
  });

  it("computes correct 7-point moving average on known data", () => {
    const points = [
      { t: "2026-03-01T00:00:00Z", v: 10 },
      { t: "2026-03-02T00:00:00Z", v: 20 },
      { t: "2026-03-03T00:00:00Z", v: 30 },
      { t: "2026-03-04T00:00:00Z", v: 40 },
      { t: "2026-03-05T00:00:00Z", v: 50 },
      { t: "2026-03-06T00:00:00Z", v: 60 },
      { t: "2026-03-07T00:00:00Z", v: 70 },
      { t: "2026-03-08T00:00:00Z", v: 80 },
      { t: "2026-03-09T00:00:00Z", v: 90 },
    ];
    const result = computeMovingAverage(points, 7);
    expect(result).toHaveLength(9);

    // Point 0: avg([10]) = 10
    expect(result[0].v).toBe(10);
    // Point 6: avg([10,20,30,40,50,60,70]) = 280/7 = 40
    expect(result[6].v).toBe(40);
    // Point 7: avg([20,30,40,50,60,70,80]) = 350/7 = 50
    expect(result[7].v).toBe(50);
    // Point 8: avg([30,40,50,60,70,80,90]) = 420/7 = 60
    expect(result[8].v).toBe(60);

    // All timestamps are preserved
    for (let i = 0; i < points.length; i++) {
      expect(result[i].t).toBe(points[i].t);
    }
  });
});
