// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo } from "react";
import { Link } from "react-router-dom";
import type { ProtocolListItem, TodaysDose } from "../../api/protocols";
import { protocolsApi } from "../../api/protocols";
import styles from "./TodaysDoses.module.css";

function computeAllTodaysDoses(protocols: ProtocolListItem[]): TodaysDose[] {
  const doses: TodaysDose[] = [];
  for (const p of protocols) {
    if (p.status !== "active") continue;
    const todayDay = Math.floor(
      (Date.now() - new Date(p.start_date).getTime()) / 86400000,
    );
    if (todayDay < 0 || todayDay >= p.duration_days) continue;

    for (const line of p.lines) {
      if (!line.schedule_pattern[todayDay]) continue;
      const dose = line.doses.find((d) => d.day_number === todayDay);
      doses.push({
        protocol_id: p.id,
        protocol_name: p.name,
        protocol_line_id: line.id,
        substance: line.substance,
        dose: line.dose,
        unit: line.unit,
        route: line.route,
        time_of_day: line.time_of_day,
        day_number: todayDay,
        status: dose?.status ?? "pending",
        dose_id: dose?.id ?? null,
      });
    }
  }
  return doses;
}

export function TodaysDoses() {
  const queryClient = useQueryClient();

  const { data: protocols, isLoading } = useQuery({
    queryKey: ["protocols"],
    queryFn: () => protocolsApi.list(),
    staleTime: 5 * 60 * 1000,
  });

  const todaysDoses = useMemo(
    () => computeAllTodaysDoses(protocols ?? []),
    [protocols],
  );

  const logDose = useMutation({
    mutationFn: (td: TodaysDose) =>
      protocolsApi.logDose(td.protocol_id, {
        protocol_line_id: td.protocol_line_id,
        day_number: td.day_number,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["protocols"] });
    },
  });

  if (isLoading) return null;
  if (!protocols || protocols.length === 0) return null;

  const allDone = todaysDoses.length > 0 && todaysDoses.every((d) => d.status !== "pending");

  return (
    <section className={`op-card ${styles.section}`}>
      <h2 className={styles.sectionTitle}>Today&rsquo;s Doses</h2>

      {todaysDoses.length === 0 && (
        <p className={styles.emptyText}>No doses scheduled today.</p>
      )}

      {allDone && (
        <p className={styles.emptyText}>All doses logged &#x2713;</p>
      )}

      {!allDone && todaysDoses.length > 0 && (
        <div className={styles.doseList}>
          {todaysDoses.map((td) => (
            <div key={`${td.protocol_line_id}-${td.day_number}`} className={styles.doseItem}>
              <div className={styles.doseInfo}>
                <span className={styles.doseSubstance}>
                  {td.substance} {td.dose}{td.unit}
                </span>
                <span className={styles.doseMeta}>
                  {td.protocol_name}{td.time_of_day ? ` \u00b7 ${td.time_of_day}` : ""}
                </span>
              </div>
              {td.status === "pending" ? (
                <button
                  type="button"
                  className="op-btn op-btn-primary op-btn-sm"
                  onClick={() => logDose.mutate(td)}
                  disabled={logDose.isPending}
                >
                  Log
                </button>
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

      <Link to="/protocols" style={{ fontSize: "var(--text-xs)", marginTop: "0.5rem", display: "inline-block" }}>
        View all protocols
      </Link>
    </section>
  );
}
