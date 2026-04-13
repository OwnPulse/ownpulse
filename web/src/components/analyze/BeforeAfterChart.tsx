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
import { CurveType } from "@unovis/ts";
import { useCallback, useMemo } from "react";
import type { WindowStats } from "../../api/stats";

interface BeforeAfterPoint {
  timestamp: number;
  value: number;
}

interface BeforeAfterChartProps {
  before: WindowStats;
  after: WindowStats;
  firstDose: string;
}

export function BeforeAfterChart({ before, after, firstDose }: BeforeAfterChartProps) {
  const chartData = useMemo<BeforeAfterPoint[]>(() => {
    const allPoints = [...before.points, ...after.points];
    return allPoints.map((p) => ({
      timestamp: new Date(p.t).getTime(),
      value: p.v,
    }));
  }, [before, after]);

  const doseTs = useMemo(() => new Date(firstDose).getTime(), [firstDose]);

  const x = useCallback((d: BeforeAfterPoint) => d.timestamp, []);
  const y = useCallback((d: BeforeAfterPoint) => d.value, []);
  const tickFormat = useCallback(
    (tick: number | Date) => new Date(tick).toLocaleDateString(),
    [],
  );
  const tooltipTemplate = useCallback(
    (d: BeforeAfterPoint) =>
      `<strong>${new Date(d.timestamp).toLocaleDateString()}</strong><br/>${d.value}`,
    [],
  );

  if (before.points.length === 0 && after.points.length === 0) {
    return <div className="op-empty">No chart data available.</div>;
  }

  return (
    <div style={{ width: "100%", height: 400 }} data-testid="ba-chart">
      <VisXYContainer<BeforeAfterPoint> data={chartData} height={400}>
        <VisLine<BeforeAfterPoint>
          x={x}
          y={y}
          curveType={CurveType.Linear}
          lineWidth={2}
          color="#3d8b8b"
        />
        <VisPlotline<BeforeAfterPoint>
          axis="x"
          value={doseTs}
          color="#c2654a"
          lineWidth={2}
          labelText="First dose"
        />
        <VisAxis<BeforeAfterPoint> type="x" tickFormat={tickFormat} />
        <VisAxis<BeforeAfterPoint> type="y" />
        <VisCrosshair<BeforeAfterPoint> template={tooltipTemplate} />
        <VisTooltip />
      </VisXYContainer>
    </div>
  );
}
