// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import React, { useMemo } from "react";
import type { ProtocolLine } from "../../api/protocols";
import styles from "./DoseStatusGrid.module.css";

interface DoseStatusGridProps {
  lines: ProtocolLine[];
  startDate: string;
  durationDays: number;
}

type CellStatus = "completed" | "missed" | "skipped" | "upcoming" | "off";

function getCellStatus(
  line: ProtocolLine,
  dayIndex: number,
  todayDayNumber: number,
): CellStatus {
  const scheduled = line.schedule_pattern[dayIndex] ?? false;
  if (!scheduled) return "off";

  const dose = line.doses.find((d) => d.day_number === dayIndex);
  if (dose) {
    if (dose.status === "completed") return "completed";
    if (dose.status === "skipped") return "skipped";
  }

  if (dayIndex < todayDayNumber) return "missed";
  return "upcoming";
}

export function DoseStatusGrid({ lines, startDate, durationDays }: DoseStatusGridProps) {
  const todayDayNumber = useMemo(
    () => Math.floor((Date.now() - new Date(startDate).getTime()) / 86400000),
    [startDate],
  );

  const dayNumbers = useMemo(
    () => Array.from({ length: durationDays }, (_, i) => i),
    [durationDays],
  );

  return (
    <div
      className={styles.grid}
      style={{ gridTemplateColumns: `10rem repeat(${durationDays}, 1.5rem)` }}
    >
      {/* Header row */}
      <div className={styles.headerCell} />
      {dayNumbers.map((d) => (
        <div key={d} className={styles.headerCell}>
          {d + 1}
        </div>
      ))}

      {/* Data rows */}
      {lines.map((line) => (
        <React.Fragment key={line.id}>
          <div className={styles.rowLabel} title={`${line.substance} ${line.dose}${line.unit}`}>
            {line.substance}
          </div>
          {dayNumbers.map((d) => {
            const status = getCellStatus(line, d, todayDayNumber);
            const isToday = d === todayDayNumber;
            return (
              <div
                key={d}
                className={`${styles.cell} ${styles[status]} ${isToday ? styles.today : ""}`}
                title={`Day ${d + 1}: ${status}`}
              />
            );
          })}
        </React.Fragment>
      ))}
    </div>
  );
}
