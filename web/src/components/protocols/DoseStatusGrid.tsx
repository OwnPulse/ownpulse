// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import type { RunDoseItem } from "../../api/protocols";
import { protocolsApi } from "../../api/protocols";
import styles from "./DoseStatusGrid.module.css";

interface DoseStatusGridProps {
  runId: string;
  durationDays: number;
}

const STATUS_SYMBOLS: Record<RunDoseItem["status"], string> = {
  completed: "✓",
  missed: "✗",
  skipped: "→",
  pending: "·",
};

// Query keys other views depend on for "what's due" and adherence numbers —
// any write here (log/skip/undo) has to invalidate all four or they'd show
// stale counts until their own next refetch.
const INVALIDATE_KEYS = (runId: string) => [
  ["todays-doses"],
  ["active-runs"],
  ["run-adherence", runId],
  ["run-doses", runId],
];

interface OpenPopover {
  dayNumber: number;
  protocolLineId: string;
  mode: "log" | "skip";
}

export function DoseStatusGrid({ runId, durationDays }: DoseStatusGridProps) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState<OpenPopover | null>(null);
  const [time, setTime] = useState("");
  const [notes, setNotes] = useState("");
  const [skipReason, setSkipReason] = useState("");

  const { data, isLoading, isError } = useQuery({
    queryKey: ["run-doses", runId],
    queryFn: () => protocolsApi.runDoses(runId, { fromDay: 0, toDay: durationDays - 1 }),
    enabled: durationDays > 0,
  });

  const closePopover = () => {
    setOpen(null);
    setTime("");
    setNotes("");
    setSkipReason("");
  };

  const invalidateAll = () => {
    for (const key of INVALIDATE_KEYS(runId)) {
      queryClient.invalidateQueries({ queryKey: key });
    }
  };

  const logMutation = useMutation({
    mutationFn: (item: RunDoseItem) => {
      const administered_at =
        time.trim() === "" ? undefined : new Date(`${item.date}T${time}`).toISOString();
      return protocolsApi.logRunDose(runId, {
        protocol_line_id: item.protocol_line_id,
        day_number: item.day_number,
        administered_at,
        notes: notes.trim() === "" ? undefined : notes.trim(),
      });
    },
    onSuccess: () => {
      invalidateAll();
      closePopover();
    },
  });

  const skipMutation = useMutation({
    mutationFn: (item: RunDoseItem) =>
      protocolsApi.skipRunDose(runId, {
        protocol_line_id: item.protocol_line_id,
        day_number: item.day_number,
        skip_reason: skipReason.trim() === "" ? undefined : skipReason.trim(),
      }),
    onSuccess: () => {
      invalidateAll();
      closePopover();
    },
  });

  const undoMutation = useMutation({
    mutationFn: (item: RunDoseItem) => {
      if (!item.dose_id) throw new Error("Missing dose id");
      return protocolsApi.deleteRunDose(runId, item.dose_id);
    },
    onSuccess: () => invalidateAll(),
  });

  if (isLoading) return <p className={styles.status}>Loading schedule...</p>;
  if (isError) return <p className={styles.status}>Error loading schedule.</p>;
  if (!data) return null;

  // Rows preserve the order lines first appear in the response (which is
  // itself ordered by the line's sort_order/day_number on the backend).
  const lineOrder: string[] = [];
  const byLine = new Map<string, Map<number, RunDoseItem>>();
  const labelByLine = new Map<string, string>();
  for (const item of data) {
    if (!byLine.has(item.protocol_line_id)) {
      byLine.set(item.protocol_line_id, new Map());
      lineOrder.push(item.protocol_line_id);
      labelByLine.set(item.protocol_line_id, item.substance);
    }
    byLine.get(item.protocol_line_id)?.set(item.day_number, item);
  }

  const dayNumbers = Array.from({ length: durationDays }, (_, i) => i);
  const today = new Date();
  const todayStr = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}`;

  return (
    <div
      className={styles.grid}
      style={{ gridTemplateColumns: `10rem repeat(${durationDays}, 2.75rem)` }}
    >
      <div className={styles.headerCell} />
      {dayNumbers.map((d) => (
        <div key={d} className={styles.headerCell}>
          {d + 1}
        </div>
      ))}

      {lineOrder.map((lineId) => {
        const days = byLine.get(lineId);
        const label = labelByLine.get(lineId) ?? "";
        return (
          <div className={styles.row} key={lineId}>
            <div className={styles.rowLabel} title={label}>
              {label}
            </div>
            {dayNumbers.map((d) => {
              const item = days?.get(d);
              if (!item) {
                return <div key={d} className={`${styles.cell} ${styles.off}`} />;
              }
              const isToday = item.date === todayStr;
              const actionable = item.status === "missed" || item.status === "pending";
              const activePopover =
                open && open.protocolLineId === item.protocol_line_id && open.dayNumber === d
                  ? open
                  : null;

              if (!actionable) {
                return (
                  <div key={d} className={styles.cellWrapper}>
                    <button
                      type="button"
                      className={`${styles.cell} ${styles[item.status]} ${isToday ? styles.today : ""}`}
                      aria-label={`Day ${d + 1}, ${item.status} — undo`}
                      onClick={() => undoMutation.mutate(item)}
                      disabled={undoMutation.isPending}
                    >
                      {STATUS_SYMBOLS[item.status]}
                    </button>
                  </div>
                );
              }

              return (
                <div key={d} className={styles.cellWrapper}>
                  <button
                    type="button"
                    className={`${styles.cell} ${styles[item.status]} ${isToday ? styles.today : ""}`}
                    aria-label={`Day ${d + 1}, ${item.status} — log or skip`}
                    onClick={() =>
                      setOpen(
                        activePopover
                          ? null
                          : { dayNumber: d, protocolLineId: item.protocol_line_id, mode: "log" },
                      )
                    }
                  >
                    {STATUS_SYMBOLS[item.status]}
                  </button>
                  {activePopover && (
                    <div className={styles.popover} role="dialog" aria-label={`Log day ${d + 1}`}>
                      <div className={styles.popoverTabs}>
                        <button
                          type="button"
                          aria-label="Switch to log form"
                          className={activePopover.mode === "log" ? styles.tabActive : styles.tab}
                          onClick={() => setOpen({ ...activePopover, mode: "log" })}
                        >
                          Log
                        </button>
                        <button
                          type="button"
                          aria-label="Switch to skip form"
                          className={activePopover.mode === "skip" ? styles.tabActive : styles.tab}
                          onClick={() => setOpen({ ...activePopover, mode: "skip" })}
                        >
                          Skip
                        </button>
                      </div>
                      {activePopover.mode === "log" ? (
                        <div className={styles.popoverForm}>
                          <label htmlFor="dose-grid-time">Time (optional)</label>
                          <input
                            id="dose-grid-time"
                            type="time"
                            value={time}
                            onChange={(e) => setTime(e.target.value)}
                          />
                          <label htmlFor="dose-grid-notes">Notes (optional)</label>
                          <textarea
                            id="dose-grid-notes"
                            value={notes}
                            onChange={(e) => setNotes(e.target.value)}
                          />
                          <div className={styles.popoverActions}>
                            <button
                              type="button"
                              className="op-btn op-btn-ghost op-btn-sm"
                              onClick={closePopover}
                            >
                              Cancel
                            </button>
                            <button
                              type="button"
                              className="op-btn op-btn-primary op-btn-sm"
                              onClick={() => logMutation.mutate(item)}
                              disabled={logMutation.isPending}
                            >
                              Log
                            </button>
                          </div>
                        </div>
                      ) : (
                        <div className={styles.popoverForm}>
                          <label htmlFor="dose-grid-skip-reason">Reason (optional)</label>
                          <input
                            id="dose-grid-skip-reason"
                            type="text"
                            value={skipReason}
                            onChange={(e) => setSkipReason(e.target.value)}
                          />
                          <div className={styles.popoverActions}>
                            <button
                              type="button"
                              className="op-btn op-btn-ghost op-btn-sm"
                              onClick={closePopover}
                            >
                              Cancel
                            </button>
                            <button
                              type="button"
                              className="op-btn op-btn-primary op-btn-sm"
                              onClick={() => skipMutation.mutate(item)}
                              disabled={skipMutation.isPending}
                            >
                              Skip
                            </button>
                          </div>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        );
      })}
    </div>
  );
}
