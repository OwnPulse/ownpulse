// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link } from "react-router-dom";
import type { TodaysDose } from "../../api/protocols";
import { protocolsApi } from "../../api/protocols";
import { invalidateDoseQueries } from "../../lib/doseInvalidation";
import styles from "./TodaysDoses.module.css";

// Shared shape between TodaysDose and MissedDoseItem — both structurally
// carry these three fields, which is all the log/skip mutations need.
interface Loggable {
  run_id: string;
  protocol_line_id: string;
  day_number: number;
}

/** `dose`/`unit` are nullable server-side — render only what's present. */
function doseAmountLabel(dose: number | null, unit: string | null): string {
  if (dose == null) return "";
  return unit != null ? ` ${dose}${unit}` : ` ${dose}`;
}

export function TodaysDoses() {
  const queryClient = useQueryClient();
  const [missedExpanded, setMissedExpanded] = useState(false);

  const {
    data: todaysDoses,
    isLoading,
    isError,
  } = useQuery({
    queryKey: ["todays-doses"],
    queryFn: () => protocolsApi.todaysDoses(),
    staleTime: 5 * 60 * 1000,
    select: (data) =>
      data.map((d) => ({
        ...d,
        status: d.status ?? ("pending" as const),
      })),
  });

  const { data: missedDoses } = useQuery({
    queryKey: ["missed-doses"],
    queryFn: () => protocolsApi.missedDoses(),
    staleTime: 5 * 60 * 1000,
  });

  const logDose = useMutation({
    mutationFn: (item: Loggable) =>
      protocolsApi.logRunDose(item.run_id, {
        protocol_line_id: item.protocol_line_id,
        day_number: item.day_number,
      }),
    onSuccess: () => invalidateDoseQueries(queryClient),
  });

  const skipDose = useMutation({
    mutationFn: (item: Loggable) =>
      protocolsApi.skipRunDose(item.run_id, {
        protocol_line_id: item.protocol_line_id,
        day_number: item.day_number,
      }),
    onSuccess: () => invalidateDoseQueries(queryClient),
  });

  if (isLoading) return null;
  if (isError) return null;

  const missedList = missedDoses ?? [];
  const missedCount = missedList.length;
  if ((!todaysDoses || todaysDoses.length === 0) && missedCount === 0) return null;

  const doses: TodaysDose[] = todaysDoses ?? [];
  const pendingCount = doses.filter((d) => d.status === "pending").length;
  const allDone = doses.length > 0 && pendingCount === 0;

  const busy = logDose.isPending || skipDose.isPending;

  return (
    <section className={`op-card ${styles.section}`}>
      <div className={styles.header}>
        <h2 className={styles.sectionTitle}>Today&rsquo;s Doses</h2>
        {pendingCount > 0 && <span className={styles.pendingBadge}>{pendingCount} pending</span>}
      </div>

      {doses.length > 0 && allDone && (
        <p className={styles.allDoneText}>
          All done <span className={styles.greenCheck}>&#x2713;</span>
        </p>
      )}

      {doses.length > 0 && !allDone && (
        <div className={styles.doseList}>
          {doses.map((td) => (
            <div
              key={`${td.protocol_line_id}-${td.day_number}`}
              className={`${styles.doseItem} ${td.status === "pending" ? styles.dosePending : ""}`}
            >
              <div className={styles.doseInfo}>
                <span className={styles.doseSubstance}>
                  {td.substance}
                  {doseAmountLabel(td.dose, td.unit)}
                </span>
                <span className={styles.doseMeta}>
                  {td.protocol_name}
                  {td.time_of_day ? ` · ${td.time_of_day}` : ""}
                </span>
              </div>
              {td.status === "pending" ? (
                <div className={styles.doseActions}>
                  <button
                    type="button"
                    className="op-btn op-btn-primary op-btn-sm"
                    onClick={() => logDose.mutate(td)}
                    disabled={busy}
                  >
                    Log
                  </button>
                  <button
                    type="button"
                    className="op-btn op-btn-ghost op-btn-sm"
                    onClick={() => skipDose.mutate(td)}
                    disabled={busy}
                  >
                    Skip
                  </button>
                </div>
              ) : (
                <span
                  className={`${styles.doseStatus} ${td.status === "completed" ? styles.statusCompleted : styles.statusSkipped}`}
                >
                  {td.status}
                </span>
              )}
            </div>
          ))}
        </div>
      )}

      {missedCount > 0 && (
        <div className={styles.missedSection}>
          <button
            type="button"
            className={styles.missedToggle}
            onClick={() => setMissedExpanded((e) => !e)}
            aria-expanded={missedExpanded}
          >
            {missedCount} missed dose{missedCount === 1 ? "" : "s"} — Review
          </button>
          {missedExpanded && (
            <div className={styles.doseList}>
              {missedList.map((md) => (
                <div key={`${md.protocol_line_id}-${md.day_number}`} className={styles.doseItem}>
                  <div className={styles.doseInfo}>
                    <span className={styles.doseSubstance}>
                      {md.substance}
                      {doseAmountLabel(md.dose, md.unit)}
                    </span>
                    <span className={styles.doseMeta}>
                      {md.protocol_name} · {md.date}
                    </span>
                  </div>
                  <div className={styles.doseActions}>
                    <button
                      type="button"
                      className="op-btn op-btn-primary op-btn-sm"
                      onClick={() => logDose.mutate(md)}
                      disabled={busy}
                    >
                      Log
                    </button>
                    <button
                      type="button"
                      className="op-btn op-btn-ghost op-btn-sm"
                      onClick={() => skipDose.mutate(md)}
                      disabled={busy}
                    >
                      Skip
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      <Link
        to="/protocols"
        style={{ fontSize: "var(--text-xs)", marginTop: "0.5rem", display: "inline-block" }}
      >
        View all protocols
      </Link>
    </section>
  );
}
