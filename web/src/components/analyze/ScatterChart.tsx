// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { VisAxis, VisCrosshair, VisScatter, VisTooltip, VisXYContainer } from "@unovis/react";
import { useCallback } from "react";

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
  const x = useCallback((d: ScatterDatum) => d.a, []);
  const y = useCallback((d: ScatterDatum) => d.b, []);
  const tooltipTemplate = useCallback(
    (d: ScatterDatum) => `${labelA}: ${d.a}<br/>${labelB}: ${d.b}`,
    [labelA, labelB],
  );

  if (data.length === 0) {
    return <div className="op-empty">No scatter data available.</div>;
  }

  return (
    <div style={{ width: "100%", height: 400 }}>
      <VisXYContainer<ScatterDatum> data={data} height={400}>
        <VisScatter<ScatterDatum> x={x} y={y} size={8} color="#c2654a" />
        <VisAxis<ScatterDatum> type="x" label={labelA} gridLine={false} />
        <VisAxis<ScatterDatum> type="y" label={labelB} gridLine={false} />
        <VisCrosshair<ScatterDatum> template={tooltipTemplate} />
        <VisTooltip />
      </VisXYContainer>
    </div>
  );
}
