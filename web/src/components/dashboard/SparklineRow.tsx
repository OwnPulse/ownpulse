// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";
import { VisLine, VisXYContainer } from "@unovis/react";
import { useMemo } from "react";
import type { DataPoint } from "../../api/explore";
import { exploreApi } from "../../api/explore";
import styles from "./SparklineRow.module.css";

const DIMENSIONS = ["energy", "mood", "focus", "recovery", "libido"] as const;
type Dimension = (typeof DIMENSIONS)[number];

interface SparklineDatum {
  x: number;
  y: number;
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

function useSparklineData() {
  const now = new Date();
  const start = new Date(now);
  start.setDate(start.getDate() - 7);
  const startStr = `${start.toISOString().slice(0, 10)}T00:00:00Z`;
  const endStr = `${now.toISOString().slice(0, 10)}T23:59:59Z`;

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

function Sparkline({ points, trend }: { points: DataPoint[]; trend: "up" | "down" | "neutral" }) {
  const data: SparklineDatum[] = useMemo(
    () =>
      points.map((p, i) => ({
        x: i,
        y: p.v,
      })),
    [points],
  );

  if (data.length === 0) {
    return <div className={styles.chartContainer} />;
  }

  const color = trend === "up" ? "#27ae60" : trend === "down" ? "#e74c3c" : "#888";

  return (
    <div className={styles.chartContainer}>
      <VisXYContainer<SparklineDatum> data={data} height={40}>
        <VisLine<SparklineDatum>
          x={(d: SparklineDatum) => d.x}
          y={(d: SparklineDatum) => d.y}
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
          <div key={d} className={styles.sparklineItem} data-testid={`sparkline-${d}`}>
            <div className={styles.sparklineHeader}>
              <span className={styles.dimensionName}>{d}</span>
              <span className={`${styles.currentValue} ${trendClass(trend)}`}>
                {currentValue != null ? currentValue : "\u2014"}
              </span>
            </div>
            <Sparkline points={points} trend={trend} />
          </div>
        );
      })}
    </div>
  );
}
