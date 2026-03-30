// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import type { ProtocolListItem } from "../api/protocols";
import { protocolsApi } from "../api/protocols";
import styles from "./Protocols.module.css";

type Filter = "active" | "completed" | "all";

function badgeClass(status: ProtocolListItem["status"]): string {
  if (status === "active") return styles.badgeActive;
  if (status === "paused") return styles.badgePaused;
  return styles.badgeCompleted;
}

function computeProgress(p: ProtocolListItem): number {
  let completed = 0;
  let total = 0;
  for (const line of p.lines) {
    for (let d = 0; d < p.duration_days; d++) {
      if (line.schedule_pattern[d]) {
        total++;
        const dose = line.doses.find((dd) => dd.day_number === d);
        if (dose && dose.status === "completed") completed++;
      }
    }
  }
  return total > 0 ? Math.round((completed / total) * 100) : 0;
}

function getNextDoseLabel(p: ProtocolListItem): string {
  const todayDay = Math.floor((Date.now() - new Date(p.start_date).getTime()) / 86400000);
  if (todayDay < 0 || todayDay >= p.duration_days) return "Protocol ended";

  for (const line of p.lines) {
    if (!line.schedule_pattern[todayDay]) continue;
    const dose = line.doses.find((d) => d.day_number === todayDay);
    if (!dose || dose.status === "pending") {
      const timeLabel = line.time_of_day ? ` (today ${line.time_of_day})` : " (today)";
      return `Next: ${line.substance} ${line.dose}${line.unit}${timeLabel}`;
    }
  }
  return "All done for today";
}

export default function Protocols() {
  const [filter, setFilter] = useState<Filter>("active");

  const {
    data: protocols,
    isLoading,
    isError,
  } = useQuery({
    queryKey: ["protocols"],
    queryFn: () => protocolsApi.list(),
  });

  const filtered = useMemo(() => {
    if (!protocols) return [];
    if (filter === "all") return protocols;
    if (filter === "active")
      return protocols.filter((p) => p.status === "active" || p.status === "paused");
    return protocols.filter((p) => p.status === "completed");
  }, [protocols, filter]);

  return (
    <main className={`op-page ${styles.page}`}>
      <div className={styles.headerRow}>
        <h1>Protocols</h1>
        <Link to="/protocols/new" className="op-btn op-btn-primary">
          New Protocol
        </Link>
      </div>

      <div className={styles.filters}>
        {(["active", "completed", "all"] as const).map((f) => (
          <button
            key={f}
            type="button"
            className={`${styles.filterBtn} ${filter === f ? styles.filterBtnActive : ""}`}
            onClick={() => setFilter(f)}
          >
            {f.charAt(0).toUpperCase() + f.slice(1)}
          </button>
        ))}
      </div>

      {isLoading && <p>Loading...</p>}
      {isError && <p>Error loading protocols.</p>}

      {!isLoading && !isError && filtered.length === 0 && (
        <p className={styles.emptyText}>
          {filter === "all" && protocols?.length === 0
            ? "No protocols yet. Create your first dosing protocol."
            : "No protocols match this filter."}
        </p>
      )}

      <div className={styles.cardList}>
        {filtered.map((p) => {
          const pct = computeProgress(p);
          return (
            <Link key={p.id} to={`/protocols/${p.id}`} className={`op-card ${styles.protocolCard}`}>
              <div className={styles.cardHeader}>
                <span className={styles.cardName}>{p.name}</span>
                <span className={`${styles.badge} ${badgeClass(p.status)}`}>
                  {p.status === "active"
                    ? "\u25CF "
                    : p.status === "paused"
                      ? "\u23F8 "
                      : "\u2713 "}
                  {p.status}
                </span>
              </div>
              <div className={styles.progressBar}>
                <div className={styles.progressFill} style={{ width: `${pct}%` }} />
              </div>
              <span className={styles.nextDose}>{getNextDoseLabel(p)}</span>
            </Link>
          );
        })}
      </div>
    </main>
  );
}
