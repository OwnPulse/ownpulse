// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { VisAxis, VisGroupedBar, VisTooltip, VisXYContainer } from "@unovis/react";
import type { LagEntry } from "../../api/stats";

interface LagChartProps {
  lags: LagEntry[];
  bestLag: number;
}

export function LagChart({ lags, bestLag }: LagChartProps) {
  if (lags.length === 0) {
    return <div className="op-empty">No lag data available.</div>;
  }

  return (
    <div style={{ width: "100%", height: 400 }}>
      <VisXYContainer<LagEntry> data={lags} height={400}>
        <VisGroupedBar<LagEntry>
          x={(d: LagEntry) => d.lag}
          y={(d: LagEntry) => d.r}
          color={(d: LagEntry) => (d.lag === bestLag ? "#c2654a" : "#3d8b8b")}
        />
        <VisAxis<LagEntry> type="x" label="Lag (days)" />
        <VisAxis<LagEntry> type="y" label="Correlation (r)" />
        <VisTooltip />
      </VisXYContainer>
    </div>
  );
}
