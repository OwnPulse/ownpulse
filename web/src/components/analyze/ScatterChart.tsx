// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { VisAxis, VisScatter, VisTooltip, VisXYContainer } from "@unovis/react";

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
  if (data.length === 0) {
    return <div className="op-empty">No scatter data available.</div>;
  }

  return (
    <div style={{ width: "100%", height: 400 }}>
      <VisXYContainer<ScatterDatum> data={data} height={400}>
        <VisScatter<ScatterDatum>
          x={(d: ScatterDatum) => d.a}
          y={(d: ScatterDatum) => d.b}
          color="#c2654a"
          size={6}
        />
        <VisAxis<ScatterDatum> type="x" label={labelA} />
        <VisAxis<ScatterDatum> type="y" label={labelB} />
        <VisTooltip />
      </VisXYContainer>
    </div>
  );
}
