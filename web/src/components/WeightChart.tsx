// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { VisAxis, VisLine, VisXYContainer } from "@unovis/react";
import { useMemo } from "react";
import type { HealthRecord } from "../api/health-records";

interface WeightChartProps {
  data: HealthRecord[];
}

function getLineColor(): string {
  const style = getComputedStyle(document.documentElement);
  return style.getPropertyValue("--chart-2").trim() || "#3d8b8b";
}

export default function WeightChart({ data }: WeightChartProps) {
  const sorted = useMemo(
    () => [...data].sort((a, b) => a.start_time.localeCompare(b.start_time)),
    [data],
  );
  const color = useMemo(() => getLineColor(), []);

  if (sorted.length === 0) {
    return <p className="op-empty">No weight data for the selected period.</p>;
  }

  const tickValues = sorted.map((_, i) => i);
  const tickFormat = (tick: number | Date): string =>
    sorted[Number(tick)]?.start_time?.slice(5, 10) ?? "";

  return (
    <div style={{ width: "100%", height: 280 }}>
      <VisXYContainer<HealthRecord> data={sorted} height={280}>
        <VisLine<HealthRecord>
          x={(_d: HealthRecord, i: number) => i}
          y={(d: HealthRecord) => d.value}
          color={color}
        />
        <VisAxis<HealthRecord>
          type="x"
          tickValues={tickValues}
          tickFormat={tickFormat}
          numTicks={Math.min(sorted.length, 12)}
        />
        <VisAxis<HealthRecord> type="y" label="kg" />
      </VisXYContainer>
    </div>
  );
}
