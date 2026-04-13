// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { VisAxis, VisCrosshair, VisGroupedBar, VisTooltip, VisXYContainer } from "@unovis/react";
import { useCallback } from "react";
import type { LagEntry } from "../../api/stats";

interface LagChartProps {
  lags: LagEntry[];
  bestLag: number;
}

export function LagChart({ lags, bestLag }: LagChartProps) {
  const x = useCallback((_d: LagEntry, i: number) => i, []);
  const y = useCallback((d: LagEntry) => d.r, []);
  const color = useCallback((d: LagEntry) => (d.lag === bestLag ? "#c2654a" : "#3d8b8b"), [bestLag]);
  const tickFormat = useCallback(
    (tick: number | Date) => {
      const idx = typeof tick === "number" ? Math.round(tick) : 0;
      return idx >= 0 && idx < lags.length ? String(lags[idx].lag) : "";
    },
    [lags],
  );
  const tooltipTemplate = useCallback(
    (d: LagEntry) => `Lag: ${d.lag} days<br/>r = ${d.r}`,
    [],
  );

  if (lags.length === 0) {
    return <div className="op-empty">No lag data available.</div>;
  }

  return (
    <div style={{ width: "100%", height: 400 }}>
      <VisXYContainer<LagEntry> data={lags} height={400}>
        <VisGroupedBar<LagEntry> x={x} y={y} color={color} />
        <VisAxis<LagEntry> type="x" label="Lag (days)" tickFormat={tickFormat} gridLine={false} />
        <VisAxis<LagEntry> type="y" label="Correlation (r)" />
        <VisCrosshair<LagEntry> template={tooltipTemplate} />
        <VisTooltip />
      </VisXYContainer>
    </div>
  );
}
