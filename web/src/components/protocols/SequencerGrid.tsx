// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import styles from "./SequencerGrid.module.css";

interface SequencerGridProps {
  lines: { substance: string; schedule_pattern: boolean[] }[];
  durationDays: number;
  editable: boolean;
  onToggleCell?: (lineIndex: number, dayIndex: number) => void;
  todayIndex?: number;
}

export default function SequencerGrid({
  lines,
  durationDays,
  editable,
  onToggleCell,
  todayIndex,
}: SequencerGridProps) {
  const cols = `120px repeat(${durationDays}, 44px)`;

  return (
    <div className={styles.wrapper}>
      <div
        className={styles.grid}
        style={{ gridTemplateColumns: cols }}
        role="grid"
        aria-label="Dosing schedule"
      >
        {/* Header row */}
        <div className={styles.cornerCell} />
        {Array.from({ length: durationDays }, (_, i) => {
          const dayNum = i + 1;
          const isWeekStart = i > 0 && i % 7 === 0;
          const weekNum = Math.floor(i / 7) + 1;
          const showWeekLabel = i % 7 === 0;
          return (
            <div
              key={i}
              className={`${styles.headerCell}${isWeekStart ? ` ${styles.weekStart}` : ""}`}
            >
              {showWeekLabel && <span className={styles.weekLabel}>W{weekNum}</span>}
              <span className={styles.dayNumber}>{dayNum}</span>
            </div>
          );
        })}

        {/* Data rows */}
        {lines.map((line, lineIdx) => (
          <>
            <div key={`label-${lineIdx}`} className={styles.label} title={line.substance}>
              {line.substance}
            </div>
            {Array.from({ length: durationDays }, (_, dayIdx) => {
              const isActive = dayIdx < line.schedule_pattern.length && line.schedule_pattern[dayIdx];
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
                <div
                  key={`${lineIdx}-${dayIdx}`}
                  className={classNames}
                  role="gridcell"
                  aria-label={`${line.substance} day ${dayIdx + 1}: ${isActive ? "active" : "inactive"}`}
                  onClick={
                    editable && onToggleCell ? () => onToggleCell(lineIdx, dayIdx) : undefined
                  }
                />
              );
            })}
          </>
        ))}
      </div>
    </div>
  );
}
