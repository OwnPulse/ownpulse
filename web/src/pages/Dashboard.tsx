// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { dashboardApi } from "../api/dashboard";
import { SparklineRow } from "../components/dashboard/SparklineRow";
import styles from "./Dashboard.module.css";

export default function Dashboard() {
  const { data, isLoading, isError } = useQuery({
    queryKey: ["dashboard-summary"],
    queryFn: dashboardApi.summary,
  });

  if (isLoading) return <main className="op-page">Loading...</main>;
  if (isError || !data) return <main className="op-page">Error loading dashboard.</main>;

  const scores = data.latest_checkin;

  return (
    <main className="op-page">
      <div className="op-page-header">
        <h1>Dashboard</h1>
        <Link to="/entry" className={`op-btn op-btn-primary ${styles.logBtn}`}>
          + Log Data
        </Link>
      </div>

      {/* Today's check-in */}
      <div className={`op-card ${styles.checkinCard}`}>
        <h2>Today&rsquo;s Check-in</h2>
        {scores ? (
          <div className={styles.checkinScores}>
            {(["energy", "mood", "focus", "recovery", "libido"] as const).map((key) => (
              <div key={key} className={styles.scoreItem}>
                <div className={styles.statValue}>{scores[key] ?? "\u2014"}</div>
                <div className={styles.statLabel}>{key}</div>
              </div>
            ))}
          </div>
        ) : (
          <p className="op-empty">
            No check-in yet today. <Link to="/entry">Log one now</Link>
          </p>
        )}
      </div>

      {/* 7-day sparklines */}
      <SparklineRow />

      {/* 7-day stats */}
      <div className={styles.statGrid}>
        {[
          { label: "Check-ins", value: data.checkin_count_7d },
          { label: "Health Records", value: data.health_record_count_7d },
          { label: "Interventions", value: data.intervention_count_7d },
          { label: "Observations", value: data.observation_count_7d },
        ].map((s) => (
          <Link key={s.label} to="/explore" className={`op-card ${styles.statLink}`}>
            <div className={styles.statValue}>{s.value}</div>
            <div className={styles.statLabel}>{s.label} (7 days)</div>
          </Link>
        ))}
      </div>

      {/* Pending shares */}
      {data.pending_friend_shares > 0 && (
        <div className={`op-card ${styles.alertCard}`}>
          <p>
            You have <strong>{data.pending_friend_shares}</strong> pending share request
            {data.pending_friend_shares > 1 ? "s" : ""}. <Link to="/friends">View</Link>
          </p>
        </div>
      )}

      {/* Latest lab */}
      {data.latest_lab_date && (
        <div className={`op-card ${styles.labCard}`}>
          <p>
            Latest lab results: <strong>{data.latest_lab_date}</strong>
          </p>
        </div>
      )}
    </main>
  );
}
