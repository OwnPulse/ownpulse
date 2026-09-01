// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";
import { VisLine, VisXYContainer } from "@unovis/react";
import { CurveType } from "@unovis/ts";
import { useCallback, useMemo } from "react";
import type { DataPoint } from "../../api/explore";
import { exploreApi } from "../../api/explore";
import { localToday } from "../../utils/datetime";
import { DIMENSION_COLORS } from "../dimensionColors.generated";
import styles from "./SparklineRow.module.css";

const DIMENSIONS = ["energy", "mood", "focus", "recovery", "libido"] as const;
type Dimension = (typeof DIMENSIONS)[number];

interface SparklineDatum {
  idx: number;
  value: number;
}

function computeTrend(points: DataPoint[]): "up" | "down" | "neutral" {
  if (points.length < 2) return "neutral";
  const firstHalf = points.slice(0, Math.floor(points.length / 2));
  const secondHalf = points.slice(Math.floor(points.length / 2));
  const avg = (pts: DataPoint[]) => pts.reduce((s, p) => s + p.v, 0) / pts.length;
  const diff = avg(secondHalf) - avg(firstHalf);
  if (diff > 0.5) return "up";
  if (diff < -0.5) return "down";
  return "neutral";
}

function trendClass(trend: "up" | "down" | "neutral"): string {
  if (trend === "up") return styles.trendUp;
  if (trend === "down") return styles.trendDown;
  return styles.trendNeutral;
}

function trendArrow(trend: "up" | "down" | "neutral"): string {
  if (trend === "up") return " \u2191";
  if (trend === "down") return " \u2193";
  return " \u2192";
}

function useSparklineData() {
  // The window boundaries should track the user's local "today", not UTC —
  // a UTC-derived end-of-day cuts off the last several hours of a west-of-UTC
  // user's actual today (and shifts the corresponding start of the 7-day
  // window). Parsed as local date parts, not `new Date(todayStr)` (UTC).
  const todayStr = localToday();
  const [y, m, d] = todayStr.split("-").map(Number);
  const start = new Date(y, m - 1, d - 7);
  const pad = (n: number) => n.toString().padStart(2, "0");
  const startDateStr = `${start.getFullYear()}-${pad(start.getMonth() + 1)}-${pad(start.getDate())}`;
  const startStr = `${startDateStr}T00:00:00Z`;
  const endStr = `${todayStr}T23:59:59Z`;

  return useQuery({
    queryKey: ["dashboard-sparklines", startStr, endStr],
    queryFn: () =>
      exploreApi.batchSeries({
        metrics: DIMENSIONS.map((d) => ({ source: "checkins", field: d })),
        start: startStr,
        end: endStr,
        resolution: "daily",
      }),
    staleTime: 5 * 60 * 1000,
  });
}

function Sparkline({ points, color }: { points: DataPoint[]; color: string }) {
  const data = useMemo<SparklineDatum[]>(
    () => points.map((p, i) => ({ idx: i, value: p.v })),
    [points],
  );
  const x = useCallback((d: SparklineDatum) => d.idx, []);
  const y = useCallback((d: SparklineDatum) => d.value, []);

  if (data.length === 0) {
    return <div className={styles.chartContainer} />;
  }

  return (
    <div className={styles.chartContainer}>
      <VisXYContainer<SparklineDatum> data={data} height={40}>
        <VisLine<SparklineDatum>
          x={x}
          y={y}
          curveType={CurveType.MonotoneX}
          lineWidth={2}
          color={color}
        />
      </VisXYContainer>
    </div>
  );
}

export function SparklineRow() {
  const { data, isLoading, isError } = useSparklineData();

  if (isLoading) {
    return (
      <div className={styles.sparklineRow} data-testid="sparkline-row-loading">
        {DIMENSIONS.map((d) => (
          <div key={d} className={styles.sparklineItem}>
            <div className={styles.sparklineHeader}>
              <span className={styles.dimensionName}>{d}</span>
              <span className={styles.currentValue}>{"\u2014"}</span>
            </div>
            <div className={styles.chartContainer} />
          </div>
        ))}
      </div>
    );
  }

  if (isError) {
    return null;
  }

  const seriesMap = new Map<string, DataPoint[]>();
  for (const s of data?.series ?? []) {
    seriesMap.set(s.field, s.points);
  }

  return (
    <div className={styles.sparklineRow} data-testid="sparkline-row">
      {DIMENSIONS.map((d: Dimension) => {
        const points = seriesMap.get(d) ?? [];
        const trend = computeTrend(points);
        const currentValue = points.length > 0 ? points[points.length - 1].v : null;

        return (
          <div
            key={d}
            className={styles.sparklineItem}
            data-testid={`sparkline-${d}`}
            style={{ borderLeftColor: DIMENSION_COLORS[d] }}
          >
            <div className={styles.sparklineHeader}>
              <span className={styles.dimensionName}>{d}</span>
              <span className={`${styles.currentValue} ${trendClass(trend)}`}>
                {currentValue != null ? `${currentValue}${trendArrow(trend)}` : "\u2014"}
              </span>
            </div>
            <Sparkline points={points} color={DIMENSION_COLORS[d]} />
          </div>
        );
      })}
    </div>
  );
}
