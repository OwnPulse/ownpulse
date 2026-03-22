// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { VisAxis, VisStackedBar, VisXYContainer } from "@unovis/react";
import type { SleepRecord } from "../api/sleep";

interface SleepChartProps {
  data: SleepRecord[];
}

// Colors for each sleep stage bar segment (order matches y accessors)
const STAGE_COLORS = ["#1a365d", "#63b3ed", "#805ad5", "#ed8936"];

// Y accessors: deep, light, REM, awake — nulls become 0 for rendering
const yAccessors = [
  (d: SleepRecord) => d.deep_minutes ?? 0,
  (d: SleepRecord) => d.light_minutes ?? 0,
  (d: SleepRecord) => d.rem_minutes ?? 0,
  (d: SleepRecord) => d.awake_minutes ?? 0,
];

// X accessor: treat date string as an index position for linear scale
function xAccessor(_d: SleepRecord, i: number): number {
  return i;
}

export default function SleepChart({ data }: SleepChartProps) {
  if (data.length === 0) {
    return <p style={{ color: "#718096" }}>No sleep data for the last 14 days.</p>;
  }

  const tickValues = data.map((_, i) => i);
  const tickFormat = (tick: number | Date): string => data[Number(tick)]?.date?.slice(5) ?? "";

  return (
    <div style={{ width: "100%", height: 280 }}>
      <VisXYContainer<SleepRecord> data={data} height={280}>
        <VisStackedBar<SleepRecord>
          x={xAccessor}
          y={yAccessors}
          color={STAGE_COLORS}
          barPadding={0.2}
          roundedCorners={2}
        />
        <VisAxis<SleepRecord>
          type="x"
          tickValues={tickValues}
          tickFormat={tickFormat}
          numTicks={data.length}
        />
        <VisAxis<SleepRecord> type="y" label="Minutes" />
      </VisXYContainer>
    </div>
  );
}
