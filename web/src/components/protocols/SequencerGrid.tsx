// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { Fragment } from "react";
import styles from "./SequencerGrid.module.css";

type DayLabelMode = "numbered" | "weekday";

const WEEKDAY_NAMES = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"] as const;

interface SequencerGridProps {
  lines: { substance: string; schedule_pattern: boolean[] }[];
  durationDays: number;
  editable: boolean;
  onToggleCell?: (lineIndex: number, dayIndex: number) => void;
  onCopyWeekForward?: (weekIndex: number) => void;
  todayIndex?: number;
  dayLabelMode?: DayLabelMode;
  startDate?: string;
}

function getWeekdayName(startDate: string, dayOffset: number): string {
  const date = new Date(startDate);
  date.setDate(date.getDate() + dayOffset);
  return WEEKDAY_NAMES[date.getDay()];
}

export default function SequencerGrid({
  lines,
  durationDays,
  editable,
  onToggleCell,
  onCopyWeekForward,
  todayIndex,
  dayLabelMode = "numbered",
  startDate,
}: SequencerGridProps) {
  const cols = `120px repeat(${durationDays}, 44px)`;
  const dayNumbers = Array.from({ length: durationDays }, (_, i) => i);
  const totalWeeks = Math.ceil(durationDays / 7);

  return (
    <div className={styles.wrapper}>
      <section
        className={styles.grid}
        style={{ gridTemplateColumns: cols }}
        aria-label="Dosing schedule"
      >
        {/* Header row */}
        <div className={styles.cornerCell} />
        {dayNumbers.map((dayIdx) => {
          const dayNum = dayIdx + 1;
          const isWeekStart = dayIdx > 0 && dayIdx % 7 === 0;
          const weekNum = Math.floor(dayIdx / 7) + 1;
          const showWeekLabel = dayIdx % 7 === 0;

          const dayLabel =
            dayLabelMode === "weekday" && startDate
              ? getWeekdayName(startDate, dayIdx)
              : `D${dayNum}`;

          return (
            <div
              key={`day-${dayNum}`}
              className={`${styles.headerCell}${isWeekStart ? ` ${styles.weekStart}` : ""}`}
            >
              {showWeekLabel && (
                <span className={styles.weekLabelRow}>
                  <span className={styles.weekLabel}>W{weekNum}</span>
                  {onCopyWeekForward && weekNum < totalWeeks && (
                    <button
                      type="button"
                      className={styles.copyWeekBtn}
                      onClick={() => onCopyWeekForward(weekNum - 1)}
                      aria-label={`Copy week ${weekNum} forward`}
                      title={`Copy week ${weekNum} to all later weeks`}
                    >
                      {"\u2192"}
                    </button>
                  )}
                </span>
              )}
              <span className={styles.dayNumber}>{dayLabel}</span>
            </div>
          );
        })}

        {/* Data rows */}
        {lines.map((line) => {
          const lineIndex = lines.indexOf(line);

          return (
            <Fragment key={`row-${line.substance}`}>
              <div className={styles.label} title={line.substance}>
                {line.substance}
              </div>
              {dayNumbers.map((dayIdx) => {
                const isActive =
                  dayIdx < line.schedule_pattern.length && line.schedule_pattern[dayIdx];
                const isToday = todayIndex !== undefined && dayIdx === todayIndex;
                const isWeekStart = dayIdx > 0 && dayIdx % 7 === 0;

                const classNames = [
                  styles.cell,
                  isActive ? styles.active : "",
                  isToday ? styles.today : "",
                  isWeekStart ? styles.weekStart : "",
                  editable ? styles.editable : "",
                ]
                  .filter(Boolean)
                  .join(" ");

                return (
                  <button
                    type="button"
                    key={`cell-${line.substance}-d${dayIdx + 1}`}
                    className={classNames}
                    aria-label={`${line.substance} day ${dayIdx + 1}: ${isActive ? "active" : "inactive"}`}
                    tabIndex={editable ? 0 : -1}
                    onClick={
                      editable && onToggleCell ? () => onToggleCell(lineIndex, dayIdx) : undefined
                    }
                  >
                    {isActive ? "\u25CF" : ""}
                  </button>
                );
              })}
            </Fragment>
          );
        })}
      </section>
    </div>
  );
}
