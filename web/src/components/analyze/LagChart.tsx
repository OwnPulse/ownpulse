// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { EChartsOption } from "echarts";
import ReactECharts from "echarts-for-react";
import { useMemo } from "react";
import type { LagEntry } from "../../api/stats";

interface LagChartProps {
  lags: LagEntry[];
  bestLag: number;
}

export function LagChart({ lags, bestLag }: LagChartProps) {
  const option = useMemo<EChartsOption>(
    () => ({
      grid: { left: 60, right: 20, top: 20, bottom: 40 },
      xAxis: {
        type: "category",
        data: lags.map((l) => l.lag),
        name: "Lag (days)",
        nameLocation: "center",
        nameGap: 25,
      },
      yAxis: { type: "value", name: "Correlation (r)", nameLocation: "center", nameGap: 40 },
      series: [
        {
          type: "bar",
          data: lags.map((l) => ({
            value: l.r,
            itemStyle: { color: l.lag === bestLag ? "#c2654a" : "#3d8b8b" },
          })),
        },
      ],
      tooltip: {
        trigger: "axis",
        formatter: (params: unknown) => {
          const p = Array.isArray(params) ? params[0] : params;
          const item = p as { name: string; value: number };
          return `Lag: ${item.name} days<br/>r = ${item.value}`;
        },
      },
    }),
    [lags, bestLag],
  );

  if (lags.length === 0) {
    return <div className="op-empty">No lag data available.</div>;
  }

  return (
    <div style={{ width: "100%", height: 400 }}>
      <ReactECharts option={option} style={{ width: "100%", height: "100%" }} />
    </div>
  );
}
