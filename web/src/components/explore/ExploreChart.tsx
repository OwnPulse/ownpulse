// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { VisAxis, VisCrosshair, VisLine, VisTooltip, VisXYContainer } from "@unovis/react";
import { useMemo, useState } from "react";
import type { SeriesResponse } from "../../api/explore";
import type { Intervention } from "../../api/interventions";
import { metricKey, useExploreStore } from "../../stores/exploreStore";

const CHART_COLOR_VARS = [
  "var(--chart-color-0)",
  "var(--chart-color-1)",
  "var(--chart-color-2)",
  "var(--chart-color-3)",
  "var(--chart-color-4)",
  "var(--chart-color-5)",
  "var(--chart-color-6)",
  "var(--chart-color-7)",
  "#332288",
  "#88CCEE",
  "#44AA99",
  "#DDCC77",
];

export const LINE_DASH_PATTERNS: (number[] | undefined)[] = [
  undefined, // solid
  [8, 4], // long dash
  [4, 4], // short dash
  [8, 4, 2, 4], // dot-dash
];

interface ExploreChartProps {
  series: SeriesResponse[];
  interventions?: Intervention[];
}

interface ChartDatum {
  timestamp: number;
  values: Record<string, number | undefined>;
}

interface InterventionMarker {
  timestamp: number;
  label: string;
  substance: string;
  dose: number;
  unit: string;
}

export function ExploreChart({ series, interventions = [] }: ExploreChartProps) {
  const hiddenMetrics = useExploreStore((s) => s.hiddenMetrics);
  const [hoveredMarker, setHoveredMarker] = useState<InterventionMarker | null>(null);
  const [tooltipPos, setTooltipPos] = useState<{ x: number; y: number } | null>(null);

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

  const markers: InterventionMarker[] = useMemo(() => {
    if (interventions.length === 0 || data.length === 0) return [];
    const minTs = data[0].timestamp;
    const maxTs = data[data.length - 1].timestamp;
    return interventions
      .map((iv) => ({
        timestamp: new Date(iv.administered_at).getTime(),
        label: `${iv.substance} ${iv.dose}${iv.unit}`,
        substance: iv.substance,
        dose: iv.dose,
        unit: iv.unit,
      }))
      .filter((m) => m.timestamp >= minTs && m.timestamp <= maxTs);
  }, [interventions, data]);

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

  const minTs = data[0].timestamp;
  const maxTs = data[data.length - 1].timestamp;
  const range = maxTs - minTs;

  return (
    <div style={{ width: "100%", height: 400, position: "relative" }}>
      <VisXYContainer<ChartDatum> data={data} height={400}>
        {visibleSeries.map((s, i) => {
          const key = `${s.source}:${s.field}`;
          const color = CHART_COLOR_VARS[i % CHART_COLOR_VARS.length];
          const isObserver = s.source === "observer_polls";
          return (
            <VisLine<ChartDatum>
              key={key}
              x={xAccessor}
              y={(d: ChartDatum) => d.values[key] ?? undefined}
              color={color}
              lineDashArray={
                isObserver ? [6, 3] : LINE_DASH_PATTERNS[i % LINE_DASH_PATTERNS.length]
              }
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
              const color = CHART_COLOR_VARS[i % CHART_COLOR_VARS.length];
              const prefix = s.source === "observer_polls" ? "(Observer) " : "";
              return `<div style="color:${color}">${prefix}${s.field}: ${val != null ? val : "N/A"} ${s.unit}</div>`;
            });
            return `<div><strong>${date}</strong>${lines.join("")}</div>`;
          }}
        />
        <VisTooltip />
      </VisXYContainer>

      {/* Intervention markers overlay */}
      {markers.length > 0 && range > 0 && (
        <svg
          data-testid="intervention-markers"
          role="img"
          aria-label={`Intervention markers: ${markers.map((m) => m.label).join(", ")}`}
          style={{
            position: "absolute",
            top: 0,
            left: 0,
            width: "100%",
            height: "100%",
            pointerEvents: "none",
          }}
          viewBox="0 0 100 100"
          preserveAspectRatio="none"
          onMouseMove={(e) => {
            const svg = e.currentTarget;
            const rect = svg.getBoundingClientRect();
            const xPct = ((e.clientX - rect.left) / rect.width) * 100;
            const threshold = 2;
            const closest = markers.find((m) => {
              const mPct = ((m.timestamp - minTs) / range) * 100;
              return Math.abs(mPct - xPct) < threshold;
            });
            if (closest) {
              setHoveredMarker(closest);
              setTooltipPos({ x: e.clientX, y: e.clientY });
              svg.style.pointerEvents = "auto";
            } else if (hoveredMarker) {
              setHoveredMarker(null);
              setTooltipPos(null);
            }
          }}
          onMouseLeave={() => {
            setHoveredMarker(null);
            setTooltipPos(null);
          }}
        >
          <title>Intervention markers</title>
          {/* Transparent full-area rect to capture mouse events */}
          <rect
            x={0}
            y={0}
            width={100}
            height={100}
            fill="transparent"
            style={{ pointerEvents: "auto" }}
          />
          {markers.map((m) => {
            const xPct = ((m.timestamp - minTs) / range) * 100;
            return (
              <line
                key={`${m.timestamp}-${m.substance}`}
                x1={xPct}
                y1={0}
                x2={xPct}
                y2={100}
                stroke="#9b59b6"
                strokeWidth={0.3}
                strokeDasharray="1.5 1"
                aria-label={`Intervention: ${m.label}`}
              />
            );
          })}
        </svg>
      )}

      {/* Intervention tooltip */}
      {hoveredMarker && tooltipPos && (
        <div
          data-testid="intervention-tooltip"
          style={{
            position: "fixed",
            left: tooltipPos.x + 12,
            top: tooltipPos.y - 10,
            background: "var(--color-surface, #fff)",
            border: "1px solid var(--color-border, #ddd)",
            borderRadius: 6,
            padding: "6px 10px",
            fontSize: "var(--text-xs, 12px)",
            zIndex: 1000,
            pointerEvents: "none",
            boxShadow: "var(--shadow-sm, 0 1px 3px rgba(0,0,0,0.1))",
          }}
        >
          <strong>{hoveredMarker.substance}</strong>
          <br />
          {hoveredMarker.dose} {hoveredMarker.unit}
        </div>
      )}
    </div>
  );
}
