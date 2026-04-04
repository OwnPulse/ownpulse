// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import type { TodaysDose } from "../../api/protocols";
import { protocolsApi } from "../../api/protocols";
import styles from "./TodaysDoses.module.css";

export function TodaysDoses() {
  const queryClient = useQueryClient();

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

  const logDose = useMutation({
    mutationFn: (td: TodaysDose) =>
      protocolsApi.logRunDose(td.run_id, {
        protocol_line_id: td.protocol_line_id,
        day_number: td.day_number,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["todays-doses"] });
      queryClient.invalidateQueries({ queryKey: ["protocols"] });
    },
  });

  const skipDose = useMutation({
    mutationFn: (td: TodaysDose) =>
      protocolsApi.skipRunDose(td.run_id, {
        protocol_line_id: td.protocol_line_id,
        day_number: td.day_number,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["todays-doses"] });
      queryClient.invalidateQueries({ queryKey: ["protocols"] });
    },
  });

  if (isLoading) return null;
  if (isError) return null;
  if (!todaysDoses || todaysDoses.length === 0) return null;

  const pendingCount = todaysDoses.filter((d) => d.status === "pending").length;
  const allDone = todaysDoses.length > 0 && pendingCount === 0;

  return (
    <section className={`op-card ${styles.section}`}>
      <div className={styles.header}>
        <h2 className={styles.sectionTitle}>Today&rsquo;s Doses</h2>
        {pendingCount > 0 && <span className={styles.pendingBadge}>{pendingCount} pending</span>}
      </div>

      {allDone && (
        <p className={styles.allDoneText}>
          All done <span className={styles.greenCheck}>&#x2713;</span>
        </p>
      )}

      {!allDone && (
        <div className={styles.doseList}>
          {todaysDoses.map((td) => (
            <div
              key={`${td.protocol_line_id}-${td.day_number}`}
              className={`${styles.doseItem} ${td.status === "pending" ? styles.dosePending : ""}`}
            >
              <div className={styles.doseInfo}>
                <span className={styles.doseSubstance}>
                  {td.substance} {td.dose}
                  {td.unit}
                </span>
                <span className={styles.doseMeta}>
                  {td.protocol_name}
                  {td.time_of_day ? ` \u00b7 ${td.time_of_day}` : ""}
                </span>
              </div>
              {td.status === "pending" ? (
                <div className={styles.doseActions}>
                  <button
                    type="button"
                    className="op-btn op-btn-primary op-btn-sm"
                    onClick={() => logDose.mutate(td)}
                    disabled={logDose.isPending || skipDose.isPending}
                  >
                    Log
                  </button>
                  <button
                    type="button"
                    className="op-btn op-btn-ghost op-btn-sm"
                    onClick={() => skipDose.mutate(td)}
                    disabled={logDose.isPending || skipDose.isPending}
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

      <Link
        to="/protocols"
        style={{ fontSize: "var(--text-xs)", marginTop: "0.5rem", display: "inline-block" }}
      >
        View all protocols
      </Link>
    </section>
  );
}
