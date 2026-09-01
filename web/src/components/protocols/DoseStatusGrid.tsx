// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { RunDoseItem } from "../../api/protocols";
import { protocolsApi } from "../../api/protocols";
import { invalidateDoseQueries } from "../../lib/doseInvalidation";
import styles from "./DoseStatusGrid.module.css";

/** Enough of a protocol line to label a grid row — from `protocol.lines`,
 *  not derived from the doses response, so a line with every day
 *  paused/unscheduled still gets a (fully "off") row instead of vanishing,
 *  and row order always matches the protocol's own `sort_order`. */
interface GridLine {
  id: string;
  substance: string;
}

interface DoseStatusGridProps {
  runId: string;
  durationDays: number;
  lines: GridLine[];
  /** Log/Skip/Undo controls only make sense for the active run — a paused
   *  or completed run still shows its history, but read-only. */
  interactive: boolean;
}

const STATUS_SYMBOLS: Record<RunDoseItem["status"], string> = {
  completed: "✓",
  missed: "✗",
  skipped: "→",
  pending: "·",
};

interface OpenPopover {
  dayNumber: number;
  protocolLineId: string;
  mode: "log" | "skip";
}

interface Anchor {
  top: number;
  left: number;
}

// Roughly the popover's rendered width (12rem min-width + padding) — used to
// keep it from opening off the right edge of the viewport.
const POPOVER_WIDTH = 208;

function anchorFromTrigger(el: HTMLElement): Anchor {
  const rect = el.getBoundingClientRect();
  const left = Math.min(rect.left, Math.max(8, window.innerWidth - POPOVER_WIDTH - 8));
  return { top: rect.bottom + 4, left };
}

/**
 * Renders into `document.body` via a portal, at fixed viewport coordinates,
 * instead of being absolutely positioned inside the grid — the grid sets
 * `overflow-x: auto`, which (per the CSS spec) forces `overflow-y` to
 * `auto` too, clipping an in-flow absolutely-positioned popover on the
 * bottom rows. Also owns Escape/outside-click dismissal and initial focus,
 * since every popover instance needs the same dialog behavior.
 */
function DosePopover({
  anchor,
  label,
  onClose,
  children,
}: {
  anchor: Anchor;
  label: string;
  onClose: () => void;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  // `onClose` (the parent's `closePopover`) is a fresh function reference
  // on every parent render (e.g. each keystroke in the notes field) — a ref
  // lets the listeners below always call the latest version without the
  // effect re-running on every render, which would re-steal focus onto the
  // first field mid-typing.
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    ref.current?.querySelector<HTMLElement>("button, input, textarea")?.focus();

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCloseRef.current();
    };
    const handlePointerDown = (e: PointerEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onCloseRef.current();
    };
    document.addEventListener("keydown", handleKeyDown);
    document.addEventListener("pointerdown", handlePointerDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      document.removeEventListener("pointerdown", handlePointerDown);
    };
    // Mount-only: this instance stays mounted for the popover's whole open
    // lifetime (React reconciles it in place at the same JSX position while
    // the same cell's popover stays open), so re-running per-render would
    // steal focus back to the first field on every keystroke.
  }, []);

  return createPortal(
    <div
      ref={ref}
      className={styles.popover}
      style={{ position: "fixed", top: anchor.top, left: anchor.left }}
      role="dialog"
      aria-label={label}
    >
      {children}
    </div>,
    document.body,
  );
}

export function DoseStatusGrid({ runId, durationDays, lines, interactive }: DoseStatusGridProps) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState<OpenPopover | null>(null);
  const [anchor, setAnchor] = useState<Anchor | null>(null);
  const [time, setTime] = useState("");
  const [notes, setNotes] = useState("");
  const [skipReason, setSkipReason] = useState("");
  const [confirmUndo, setConfirmUndo] = useState<{
    protocolLineId: string;
    dayNumber: number;
  } | null>(null);

  const { data, isLoading, isError } = useQuery({
    queryKey: ["run-doses", runId],
    // No explicit day range — matches the server (and iOS's) default of
    // `0..=min(today, duration-1)`, so a future day simply isn't in the
    // response and renders as an inert "off" cell rather than an
    // actionable one the backend will then 400 on logging.
    queryFn: () => protocolsApi.runDoses(runId),
  });

  const closePopover = () => {
    setOpen(null);
    setAnchor(null);
    setTime("");
    setNotes("");
    setSkipReason("");
    logMutation.reset();
    skipMutation.reset();
  };

  const openPopover = (e: React.MouseEvent<HTMLButtonElement>, item: RunDoseItem) => {
    setAnchor(anchorFromTrigger(e.currentTarget));
    setOpen({ dayNumber: item.day_number, protocolLineId: item.protocol_line_id, mode: "log" });
    setTime("");
    setNotes("");
    setSkipReason("");
    logMutation.reset();
    skipMutation.reset();
  };

  const invalidateAll = () => invalidateDoseQueries(queryClient);

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

  // Keyed by the protocol's own line list (sort_order), not first-appearance
  // in the doses response — a line unscheduled on day 0 would otherwise sort
  // last, and a line with every day paused/unscheduled wouldn't appear in
  // the response at all and would vanish from the grid entirely.
  const byLine = new Map<string, Map<number, RunDoseItem>>();
  for (const item of data) {
    if (!byLine.has(item.protocol_line_id)) {
      byLine.set(item.protocol_line_id, new Map());
    }
    byLine.get(item.protocol_line_id)?.set(item.day_number, item);
  }

  const dayNumbers = Array.from({ length: durationDays }, (_, i) => i);
  const today = new Date();
  const todayStr = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}`;

  const isConfirmingUndo = (item: RunDoseItem) =>
    confirmUndo?.protocolLineId === item.protocol_line_id &&
    confirmUndo?.dayNumber === item.day_number;

  const handleUndoClick = (item: RunDoseItem) => {
    if (isConfirmingUndo(item)) {
      undoMutation.mutate(item);
      setConfirmUndo(null);
    } else {
      setConfirmUndo({ protocolLineId: item.protocol_line_id, dayNumber: item.day_number });
    }
  };

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

      {lines.map((line) => {
        const days = byLine.get(line.id);
        return (
          <div className={styles.row} key={line.id}>
            <div className={styles.rowLabel} title={line.substance}>
              {line.substance}
            </div>
            {dayNumbers.map((d) => {
              const item = days?.get(d);
              if (!item) {
                return <div key={d} className={`${styles.cell} ${styles.off}`} />;
              }
              const isToday = item.date === todayStr;

              if (!interactive) {
                return (
                  <div key={d} className={styles.cellWrapper}>
                    <div
                      role="img"
                      className={`${styles.cell} ${styles[item.status]} ${isToday ? styles.today : ""}`}
                      aria-label={`Day ${d + 1}, ${item.status}`}
                    >
                      {STATUS_SYMBOLS[item.status]}
                    </div>
                  </div>
                );
              }

              const actionable = item.status === "missed" || item.status === "pending";
              const activePopover =
                open && open.protocolLineId === item.protocol_line_id && open.dayNumber === d
                  ? open
                  : null;

              if (!actionable) {
                const confirming = isConfirmingUndo(item);
                return (
                  <div key={d} className={styles.cellWrapper}>
                    <button
                      type="button"
                      className={`${styles.cell} ${styles[item.status]} ${isToday ? styles.today : ""} ${confirming ? styles.confirming : ""}`}
                      aria-label={`Day ${d + 1}, ${item.status} — ${confirming ? "confirm undo" : "undo"}`}
                      onClick={() => handleUndoClick(item)}
                      disabled={undoMutation.isPending}
                    >
                      {confirming ? "?" : STATUS_SYMBOLS[item.status]}
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
                    onClick={(e) => (activePopover ? closePopover() : openPopover(e, item))}
                  >
                    {STATUS_SYMBOLS[item.status]}
                  </button>
                  {activePopover && anchor && (
                    <DosePopover anchor={anchor} label={`Log day ${d + 1}`} onClose={closePopover}>
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
                          {logMutation.isError && (
                            <p className={styles.popoverError} role="alert">
                              Error: {logMutation.error.message}
                            </p>
                          )}
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
                          {skipMutation.isError && (
                            <p className={styles.popoverError} role="alert">
                              Error: {skipMutation.error.message}
                            </p>
                          )}
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
                    </DosePopover>
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
