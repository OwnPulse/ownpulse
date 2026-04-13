// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import {
  VisAxis,
  VisBrush,
  VisCrosshair,
  VisLine,
  VisPlotline,
  VisTooltip,
  VisXYContainer,
} from "@unovis/react";
import { CurveType, PlotlineLineStylePresets } from "@unovis/ts";
import { useCallback, useMemo } from "react";
import type { SeriesResponse } from "../../api/explore";
import type { Intervention } from "../../api/interventions";
import { metricKey, useExploreStore } from "../../stores/exploreStore";
import { CHART_COLORS, INTERVENTION_COLOR, LINE_STYLES } from "./chartColors";

/** Unified data point: one entry per timestamp, with per-series values as dynamic keys. */
interface ChartDataPoint {
  timestamp: number;
  [seriesKey: string]: number | undefined;
}

/** Map LINE_STYLES values to unovis dash arrays. */
function toDashArray(style: "solid" | "dashed" | number[]): number[] | undefined {
  if (style === "solid") return undefined;
  if (style === "dashed") return [6, 4];
  return style;
}

interface ExploreChartProps {
  series: SeriesResponse[];
  interventions?: Intervention[];
}

export function ExploreChart({ series, interventions = [] }: ExploreChartProps) {
  const hiddenMetrics = useExploreStore((s) => s.hiddenMetrics);
  const hiddenSubstances = useExploreStore((s) => s.hiddenSubstances);
  const setZoomRange = useExploreStore((s) => s.setZoomRange);

  const visibleSeries = useMemo(
    () => series.filter((s) => !hiddenMetrics.has(metricKey({ source: s.source, field: s.field }))),
    [series, hiddenMetrics],
  );

  const visibleInterventions = useMemo(
    () => interventions.filter((iv) => !hiddenSubstances.includes(iv.substance)),
    [interventions, hiddenSubstances],
  );

  /** Merge all series into a single flat array keyed by timestamp. */
  const { chartData, seriesKeys } = useMemo(() => {
    const tsMap = new Map<number, ChartDataPoint>();
    const keys: string[] = [];

    for (const s of visibleSeries) {
      const key = `${s.source}:${s.field}`;
      keys.push(key);
      for (const p of s.points) {
        const ts = new Date(p.t).getTime();
        let entry = tsMap.get(ts);
        if (!entry) {
          entry = { timestamp: ts };
          tsMap.set(ts, entry);
        }
        entry[key] = p.v;
      }
    }

    const sorted = Array.from(tsMap.values()).sort((a, b) => a.timestamp - b.timestamp);
    return { chartData: sorted, seriesKeys: keys };
  }, [visibleSeries]);

  const x = useCallback((d: ChartDataPoint) => d.timestamp, []);

  const tickFormat = useCallback((tick: number | Date) => {
    const d = tick instanceof Date ? tick : new Date(tick);
    return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  }, []);

  /** Build crosshair tooltip showing all series values at a given point. */
  const tooltipTemplate = useCallback(
    (d: ChartDataPoint) => {
      const date = new Date(d.timestamp).toLocaleString();
      const lines = visibleSeries
        .map((s) => {
          const key = `${s.source}:${s.field}`;
          const val = d[key];
          const color = CHART_COLORS[seriesKeys.indexOf(key) % CHART_COLORS.length];
          return `<div><span style="color:${color}">\u25CF</span> ${s.field} (${s.unit}): ${val != null ? val : "N/A"}</div>`;
        })
        .join("");
      return `<div><strong>${date}</strong>${lines}</div>`;
    },
    [visibleSeries, seriesKeys],
  );

  const onBrushEnd = useCallback(
    (selection: [number, number] | undefined) => {
      if (selection) {
        setZoomRange(selection);
      }
    },
    [setZoomRange],
  );

  if (visibleSeries.length === 0 || visibleSeries.every((s) => s.points.length === 0)) {
    return (
      <div className="op-empty">
        {series.length === 0
          ? "Select metrics from the picker to start exploring."
          : "No data available for the selected metrics and range."}
      </div>
    );
  }

  // Collect unique units for dual y-axis labels
  const unitOrder: string[] = [];
  for (const s of visibleSeries) {
    if (!unitOrder.includes(s.unit)) unitOrder.push(s.unit);
  }

  return (
    <div style={{ width: "100%", minHeight: 300, height: "50vh" }}>
      <VisXYContainer<ChartDataPoint> data={chartData} height="50vh">
        {visibleSeries.map((s) => {
          const key = `${s.source}:${s.field}`;
          const colorIdx = seriesKeys.indexOf(key);
          const color = CHART_COLORS[colorIdx % CHART_COLORS.length];
          const dashArray = toDashArray(LINE_STYLES[colorIdx % LINE_STYLES.length]);
          return (
            <VisLine<ChartDataPoint>
              key={key}
              x={x}
              y={(d: ChartDataPoint) => d[key] as number | undefined}
              curveType={CurveType.MonotoneX}
              lineWidth={2.5}
              color={color}
              lineDashArray={dashArray}
            />
          );
        })}
        {visibleInterventions.map((iv) => (
          <VisPlotline<ChartDataPoint>
            key={`iv-${iv.id}`}
            axis="x"
            value={new Date(iv.administered_at).getTime()}
            color={INTERVENTION_COLOR}
            lineWidth={1.5}
            lineStyle={PlotlineLineStylePresets.Dash}
            labelText={iv.substance}
            labelColor={INTERVENTION_COLOR}
          />
        ))}
        <VisAxis<ChartDataPoint>
          type="x"
          tickFormat={tickFormat}
          gridLine={false}
          tickTextFontSize="11px"
        />
        <VisAxis<ChartDataPoint> type="y" label={unitOrder[0] ?? ""} tickTextFontSize="11px" />
        {unitOrder.length >= 2 && (
          <VisAxis<ChartDataPoint>
            type="y"
            position="right"
            label={unitOrder[1]}
            gridLine={false}
            tickTextFontSize="11px"
          />
        )}
        <VisCrosshair<ChartDataPoint> template={tooltipTemplate} />
        <VisTooltip />
        <VisBrush<ChartDataPoint> onBrushEnd={onBrushEnd} draggable />
      </VisXYContainer>
    </div>
  );
}
