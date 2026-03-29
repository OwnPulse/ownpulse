// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import {
  VisAxis,
  VisCrosshair,
  VisLine,
  VisPlotline,
  VisTooltip,
  VisXYContainer,
} from "@unovis/react";
import { useMemo } from "react";
import type { WindowStats } from "../../api/stats";

interface BeforeAfterChartProps {
  before: WindowStats;
  after: WindowStats;
  firstDose: string;
}

interface ChartDatum {
  timestamp: number;
  value: number;
}

export function BeforeAfterChart({ before, after, firstDose }: BeforeAfterChartProps) {
  const { beforeData, afterData, doseTimestamp } = useMemo(() => {
    const bd: ChartDatum[] = before.points.map((p) => ({
      timestamp: new Date(p.t).getTime(),
      value: p.v,
    }));
    const ad: ChartDatum[] = after.points.map((p) => ({
      timestamp: new Date(p.t).getTime(),
      value: p.v,
    }));
    return {
      beforeData: bd,
      afterData: ad,
      doseTimestamp: new Date(firstDose).getTime(),
    };
  }, [before, after, firstDose]);

  const allData = [...beforeData, ...afterData];

  if (allData.length === 0) {
    return <div className="op-empty">No chart data available.</div>;
  }

  return (
    <div style={{ width: "100%", height: 400 }} data-testid="ba-chart">
      <VisXYContainer<ChartDatum> data={allData} height={400}>
        <VisLine<ChartDatum>
          x={(d: ChartDatum) => d.timestamp}
          y={(d: ChartDatum) => d.value}
          color="#3d8b8b"
        />
        <VisPlotline<ChartDatum>
          value={doseTimestamp}
          color="#c2654a"
          lineWidth={2}
          labelText="First dose"
        />
        <VisAxis<ChartDatum>
          type="x"
          tickFormat={(v: number | Date) => {
            const d = new Date(Number(v));
            return `${String(d.getMonth() + 1).padStart(2, "0")}/${String(d.getDate()).padStart(2, "0")}`;
          }}
        />
        <VisAxis<ChartDatum> type="y" />
        <VisCrosshair<ChartDatum>
          template={(d: ChartDatum) => {
            const date = new Date(d.timestamp).toLocaleDateString();
            return `<div><strong>${date}</strong><div>${d.value}</div></div>`;
          }}
        />
        <VisTooltip />
      </VisXYContainer>
    </div>
  );
}
