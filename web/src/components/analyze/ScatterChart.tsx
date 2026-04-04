// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { EChartsOption } from "echarts";
import ReactECharts from "echarts-for-react";
import { useMemo } from "react";

interface ScatterDatum {
  a: number;
  b: number;
  t: string;
}

interface ScatterChartProps {
  data: ScatterDatum[];
  labelA: string;
  labelB: string;
}

export function ScatterChart({ data, labelA, labelB }: ScatterChartProps) {
  const option = useMemo<EChartsOption>(
    () => ({
      grid: { left: 50, right: 20, top: 20, bottom: 40 },
      xAxis: { type: "value", name: labelA, nameLocation: "center", nameGap: 25 },
      yAxis: { type: "value", name: labelB, nameLocation: "center", nameGap: 35 },
      series: [
        {
          type: "scatter",
          data: data.map((d) => [d.a, d.b]),
          symbolSize: 8,
          itemStyle: { color: "#c2654a" },
        },
      ],
      tooltip: {
        trigger: "item",
        formatter: (params: unknown) => {
          const p = params as { value: [number, number] };
          return `${labelA}: ${p.value[0]}<br/>${labelB}: ${p.value[1]}`;
        },
      },
    }),
    [data, labelA, labelB],
  );

  if (data.length === 0) {
    return <div className="op-empty">No scatter data available.</div>;
  }

  return (
    <div style={{ width: "100%", height: 400 }}>
      <ReactECharts option={option} style={{ width: "100%", height: "100%" }} />
    </div>
  );
}
