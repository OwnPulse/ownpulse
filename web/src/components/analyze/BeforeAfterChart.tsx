// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { EChartsOption } from "echarts";
import ReactECharts from "echarts-for-react";
import { useMemo } from "react";
import type { WindowStats } from "../../api/stats";

interface BeforeAfterChartProps {
  before: WindowStats;
  after: WindowStats;
  firstDose: string;
}

export function BeforeAfterChart({ before, after, firstDose }: BeforeAfterChartProps) {
  const option = useMemo<EChartsOption>(() => {
    const allPoints = [...before.points, ...after.points];
    if (allPoints.length === 0) return {};

    const timestamps = allPoints.map((p) => new Date(p.t).getTime());
    const values = allPoints.map((p) => p.v);
    const doseTs = new Date(firstDose).getTime();

    return {
      grid: { left: 50, right: 20, top: 20, bottom: 40 },
      xAxis: {
        type: "time",
        data: timestamps,
      },
      yAxis: { type: "value" },
      series: [
        {
          type: "line",
          data: timestamps.map((t, i) => [t, values[i]]),
          smooth: false,
          symbol: "circle",
          symbolSize: 4,
          lineStyle: { color: "#3d8b8b", width: 2 },
          itemStyle: { color: "#3d8b8b" },
          markLine: {
            silent: true,
            symbol: "none",
            data: [
              {
                xAxis: doseTs,
                label: { formatter: "First dose", position: "insideEndTop" },
                lineStyle: { color: "#c2654a", width: 2, type: "solid" },
              },
            ],
          },
        },
      ],
      tooltip: {
        trigger: "axis",
        formatter: (params: unknown) => {
          const p = Array.isArray(params) ? params[0] : params;
          const item = p as { value: [number, number] };
          const date = new Date(item.value[0]).toLocaleDateString();
          return `<strong>${date}</strong><br/>${item.value[1]}`;
        },
      },
    };
  }, [before, after, firstDose]);

  if (before.points.length === 0 && after.points.length === 0) {
    return <div className="op-empty">No chart data available.</div>;
  }

  return (
    <div style={{ width: "100%", height: 400 }} data-testid="ba-chart">
      <ReactECharts option={option} style={{ width: "100%", height: "100%" }} />
    </div>
  );
}
