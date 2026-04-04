// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";
import type { EChartsOption } from "echarts";
import ReactECharts from "echarts-for-react";
import { useMemo } from "react";
import type { DataPoint } from "../../api/explore";
import { exploreApi } from "../../api/explore";
import styles from "./SparklineRow.module.css";

const DIMENSIONS = ["energy", "mood", "focus", "recovery", "libido"] as const;
type Dimension = (typeof DIMENSIONS)[number];

const DIMENSION_COLORS: Record<Dimension, string> = {
  energy: "#c49a3c",
  mood: "#c2654a",
  focus: "#3d8b8b",
  recovery: "#5a8a5a",
  libido: "#7b61c2",
};

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

function Sparkline({ points, color }: { points: DataPoint[]; color: string }) {
  const data = useMemo(() => points.map((p) => p.v), [points]);

  if (data.length === 0) {
    return <div className={styles.chartContainer} />;
  }

  const option: EChartsOption = {
    grid: { left: 0, right: 0, top: 2, bottom: 2 },
    xAxis: { show: false, type: "category", data: data.map((_, i) => i) },
    yAxis: { show: false, min: "dataMin", max: "dataMax" },
    series: [
      {
        type: "line",
        data,
        smooth: 0.4,
        symbol: "none",
        lineStyle: { width: 2, color },
        areaStyle: { color, opacity: 0.08 },
      },
    ],
  };

  return (
    <div className={styles.chartContainer}>
      <ReactECharts
        option={option}
        style={{ width: "100%", height: 40 }}
        opts={{ renderer: "svg" }}
      />
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
