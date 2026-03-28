// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { VisAxis, VisCrosshair, VisLine, VisTooltip, VisXYContainer } from "@unovis/react";
import { useMemo } from "react";
import type { SeriesResponse } from "../../api/explore";
import { metricKey, useExploreStore } from "../../stores/exploreStore";

const CHART_COLORS = [
  "#c2654a",
  "#3d8b8b",
  "#c49a3c",
  "#5a8a5a",
  "#9b59b6",
  "#1abc9c",
  "#f39c12",
  "#2980b9",
  "#d35400",
  "#27ae60",
  "#8e44ad",
  "#e74c3c",
];

interface ExploreChartProps {
  series: SeriesResponse[];
}

interface ChartDatum {
  timestamp: number;
  values: Record<string, number | undefined>;
}

export function ExploreChart({ series }: ExploreChartProps) {
  const hiddenMetrics = useExploreStore((s) => s.hiddenMetrics);

  const visibleSeries = useMemo(
    () => series.filter((s) => !hiddenMetrics.has(metricKey({ source: s.source, field: s.field }))),
    [series, hiddenMetrics],
  );

  const { data, units } = useMemo(() => {
    const timestampSet = new Set<number>();
    const unitSet = new Set<string>();

    for (const s of visibleSeries) {
      unitSet.add(s.unit);
      for (const p of s.points) {
        timestampSet.add(new Date(p.t).getTime());
      }
    }

    const timestamps = [...timestampSet].sort((a, b) => a - b);
    const lookup: Record<string, Map<number, number>> = {};

    for (const s of visibleSeries) {
      const key = `${s.source}:${s.field}`;
      const map = new Map<number, number>();
      for (const p of s.points) {
        map.set(new Date(p.t).getTime(), p.v);
      }
      lookup[key] = map;
    }

    const chartData: ChartDatum[] = timestamps.map((ts) => {
      const values: Record<string, number | undefined> = {};
      for (const s of visibleSeries) {
        const key = `${s.source}:${s.field}`;
        values[key] = lookup[key].get(ts);
      }
      return { timestamp: ts, values };
    });

    return { data: chartData, units: [...unitSet] };
  }, [visibleSeries]);

  if (visibleSeries.length === 0 || data.length === 0) {
    return (
      <div className="op-empty">
        {series.length === 0
          ? "Select metrics from the picker to start exploring."
          : "No data available for the selected metrics and range."}
      </div>
    );
  }

  const xAccessor = (d: ChartDatum) => d.timestamp;

  const formatDate = (v: number | Date): string => {
    const d = new Date(Number(v));
    const month = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${month}/${day}`;
  };

  const hasDualAxes = units.length === 2;

  return (
    <div style={{ width: "100%", height: 400 }}>
      <VisXYContainer<ChartDatum> data={data} height={400}>
        {visibleSeries.map((s, i) => {
          const key = `${s.source}:${s.field}`;
          const color = CHART_COLORS[i % CHART_COLORS.length];
          return (
            <VisLine<ChartDatum>
              key={key}
              x={xAccessor}
              y={(d: ChartDatum) => d.values[key] ?? undefined}
              color={color}
            />
          );
        })}
        <VisAxis<ChartDatum> type="x" tickFormat={formatDate} />
        <VisAxis<ChartDatum> type="y" label={units[0] ?? ""} />
        {hasDualAxes && <VisAxis<ChartDatum> type="y" label={units[1]} position="right" />}
        <VisCrosshair<ChartDatum>
          template={(d: ChartDatum) => {
            const date = new Date(d.timestamp).toLocaleDateString();
            const lines = visibleSeries.map((s, i) => {
              const key = `${s.source}:${s.field}`;
              const val = d.values[key];
              const color = CHART_COLORS[i % CHART_COLORS.length];
              return `<div style="color:${color}">${s.field}: ${val != null ? val : "N/A"} ${s.unit}</div>`;
            });
            return `<div><strong>${date}</strong>${lines.join("")}</div>`;
          }}
        />
        <VisTooltip />
      </VisXYContainer>
    </div>
  );
}
