// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { EChartsOption } from "echarts";
import ReactECharts from "echarts-for-react";
import { useCallback, useMemo } from "react";
import type { SeriesResponse } from "../../api/explore";
import type { Intervention } from "../../api/interventions";
import { useTheme } from "../../hooks/useTheme";
import { metricKey, useExploreStore } from "../../stores/exploreStore";
import { CHART_COLORS, INTERVENTION_COLOR, LINE_STYLES } from "./chartColors";

interface ExploreChartProps {
  series: SeriesResponse[];
  interventions?: Intervention[];
}

export function ExploreChart({ series, interventions = [] }: ExploreChartProps) {
  const hiddenMetrics = useExploreStore((s) => s.hiddenMetrics);
  const hiddenSubstances = useExploreStore((s) => s.hiddenSubstances);
  const setZoomRange = useExploreStore((s) => s.setZoomRange);
  const { resolvedTheme } = useTheme();
  const isDark = resolvedTheme === "dark";

  const visibleSeries = useMemo(
    () => series.filter((s) => !hiddenMetrics.has(metricKey({ source: s.source, field: s.field }))),
    [series, hiddenMetrics],
  );

  const visibleInterventions = useMemo(
    () => interventions.filter((iv) => !hiddenSubstances.includes(iv.substance)),
    [interventions, hiddenSubstances],
  );

  const option = useMemo((): EChartsOption => {
    if (visibleSeries.length === 0) return {};

    // Collect unique units preserving order
    const unitOrder: string[] = [];
    for (const s of visibleSeries) {
      if (!unitOrder.includes(s.unit)) unitOrder.push(s.unit);
    }
    const hasRightAxis = unitOrder.length >= 2;

    // Map each series to a yAxisIndex
    const yAxisIndexForUnit = (unit: string): number => {
      const idx = unitOrder.indexOf(unit);
      return idx <= 1 ? idx : idx % 2;
    };

    const echartsSeries: EChartsOption["series"] = visibleSeries.map((s, i) => {
      const base = {
        type: "line" as const,
        name: `${s.field} (${s.unit})`,
        data: s.points.map((p) => [new Date(p.t).getTime(), p.v]),
        smooth: 0.3,
        symbol: "none",
        lineStyle: {
          width: 2.5,
          color: CHART_COLORS[i % CHART_COLORS.length],
          type: LINE_STYLES[i % LINE_STYLES.length],
        },
        itemStyle: { color: CHART_COLORS[i % CHART_COLORS.length] },
        yAxisIndex: yAxisIndexForUnit(s.unit),
      };

      // Attach intervention markLines to the first series
      if (i === 0 && visibleInterventions.length > 0) {
        return {
          ...base,
          markLine: {
            silent: true,
            symbol: "none",
            data: visibleInterventions.map((iv) => ({
              xAxis: new Date(iv.administered_at).getTime(),
              label: {
                formatter: iv.substance,
                position: "start" as const,
                fontSize: 10,
                color: INTERVENTION_COLOR,
              },
              lineStyle: {
                color: INTERVENTION_COLOR,
                type: "dashed" as const,
                width: 1.5,
              },
            })),
          },
        };
      }

      return base;
    });

    const yAxis: EChartsOption["yAxis"] = [
      {
        type: "value",
        name: unitOrder[0] ?? "",
        nameTextStyle: { color: isDark ? "#aaa" : "#666", fontSize: 11 },
        axisLabel: { color: isDark ? "#aaa" : "#666", fontSize: 11 },
        axisLine: { show: false },
        splitLine: {
          lineStyle: { color: isDark ? "rgba(255,255,255,0.08)" : "rgba(0,0,0,0.06)" },
        },
      },
    ];

    if (hasRightAxis) {
      yAxis.push({
        type: "value",
        name: unitOrder[1],
        nameTextStyle: { color: isDark ? "#aaa" : "#666", fontSize: 11 },
        axisLabel: { color: isDark ? "#aaa" : "#666", fontSize: 11 },
        axisLine: { show: false },
        splitLine: { show: false },
        position: "right",
      });
    }

    return {
      grid: {
        left: 56,
        right: hasRightAxis ? 56 : 20,
        top: 20,
        bottom: 56,
        containLabel: false,
      },
      xAxis: {
        type: "time",
        axisLabel: { color: isDark ? "#aaa" : "#666", fontSize: 11 },
        axisLine: { lineStyle: { color: isDark ? "#444" : "#ddd" } },
        splitLine: { show: false },
      },
      yAxis,
      series: echartsSeries,
      tooltip: {
        trigger: "axis",
        backgroundColor: isDark ? "#2a2a28" : "#ffffff",
        borderColor: isDark ? "#3a3a38" : "#e0e0e0",
        textStyle: { color: isDark ? "#eeeeea" : "#1e1e1c", fontSize: 12 },
        formatter: (params: unknown) => {
          if (!Array.isArray(params) || params.length === 0) return "";
          const first = params[0] as { axisValueLabel?: string };
          const header = first.axisValueLabel ?? "";
          const lines = (
            params as Array<{ marker?: string; seriesName?: string; value?: [number, number] }>
          )
            .map((p) => {
              const val = p.value?.[1];
              return `<div>${p.marker ?? ""} ${p.seriesName}: ${val != null ? val : "N/A"}</div>`;
            })
            .join("");
          return `<div><strong>${header}</strong>${lines}</div>`;
        },
      },
      dataZoom: [
        { type: "inside", xAxisIndex: 0 },
        {
          type: "slider",
          xAxisIndex: 0,
          bottom: 8,
          height: 20,
          borderColor: "transparent",
          backgroundColor: isDark ? "rgba(255,255,255,0.05)" : "rgba(0,0,0,0.03)",
          fillerColor: isDark ? "rgba(194,101,74,0.2)" : "rgba(194,101,74,0.15)",
          handleStyle: { color: "#c2654a" },
          textStyle: { color: isDark ? "#aaa" : "#666" },
        },
      ],
      animation: true,
      animationDuration: 300,
    };
  }, [visibleSeries, visibleInterventions, isDark]);

  const onDataZoom = useCallback(
    (params: Record<string, unknown>) => {
      const batch = params.batch as Array<{ start?: number; end?: number }> | undefined;
      if (batch && batch.length > 0) {
        const { start, end } = batch[0];
        if (start != null && end != null) {
          setZoomRange([start, end]);
        }
      } else if (params.start != null && params.end != null) {
        setZoomRange([params.start as number, params.end as number]);
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

  return (
    <div style={{ width: "100%", minHeight: 300, height: "50vh" }}>
      <ReactECharts
        option={option}
        style={{ width: "100%", height: "100%" }}
        notMerge={true}
        onEvents={{ dataZoom: onDataZoom }}
      />
    </div>
  );
}
