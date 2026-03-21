// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { dashboardApi } from "../api/dashboard";

const cardStyle: React.CSSProperties = {
  background: "var(--color-surface)",
  borderRadius: "var(--radius-md)",
  boxShadow: "var(--shadow-sm)",
  padding: "1.25rem",
  border: "1px solid var(--color-border)",
};

const statGrid: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(auto-fit, minmax(140px, 1fr))",
  gap: "1rem",
  marginBottom: "1.5rem",
};

const statValue: React.CSSProperties = {
  fontSize: "2rem",
  fontWeight: 700,
  color: "var(--color-text)",
  lineHeight: 1,
};

const statLabel: React.CSSProperties = {
  fontSize: "0.8125rem",
  color: "var(--color-text-muted)",
  marginTop: "0.25rem",
};

export default function Dashboard() {
  const { data, isLoading, isError } = useQuery({
    queryKey: ["dashboard-summary"],
    queryFn: dashboardApi.summary,
  });

  if (isLoading) return <main style={{ padding: "1.5rem" }}>Loading...</main>;
  if (isError || !data) return <main style={{ padding: "1.5rem" }}>Error loading dashboard.</main>;

  const scores = data.latest_checkin;

  return (
    <main style={{ padding: "1.5rem", fontFamily: "var(--font-family)" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1.5rem" }}>
        <h1 style={{ margin: 0, fontSize: "1.5rem", color: "var(--color-text)" }}>Dashboard</h1>
        <Link
          to="/entry"
          style={{
            padding: "0.5rem 1rem",
            background: "var(--color-primary)",
            color: "#fff",
            borderRadius: "var(--radius-sm)",
            textDecoration: "none",
            fontSize: "0.875rem",
            fontWeight: 600,
          }}
        >
          + Log Data
        </Link>
      </div>

      {/* Today's check-in */}
      <div style={{ ...cardStyle, marginBottom: "1.5rem" }}>
        <h2 style={{ margin: "0 0 0.75rem", fontSize: "1rem", color: "var(--color-text)" }}>
          Today&rsquo;s Check-in
        </h2>
        {scores ? (
          <div style={{ display: "flex", gap: "1.5rem", flexWrap: "wrap" }}>
            {(["energy", "mood", "focus", "recovery", "libido"] as const).map((key) => (
              <div key={key} style={{ textAlign: "center" }}>
                <div style={statValue}>{scores[key] ?? "\u2014"}</div>
                <div style={statLabel}>{key}</div>
              </div>
            ))}
          </div>
        ) : (
          <p style={{ margin: 0, color: "var(--color-text-muted)" }}>
            No check-in yet today.{" "}
            <Link to="/entry" style={{ color: "var(--color-primary)" }}>Log one now</Link>
          </p>
        )}
      </div>

      {/* 7-day stats */}
      <div style={statGrid}>
        {[
          { label: "Check-ins", value: data.checkin_count_7d },
          { label: "Health Records", value: data.health_record_count_7d },
          { label: "Interventions", value: data.intervention_count_7d },
          { label: "Observations", value: data.observation_count_7d },
        ].map((s) => (
          <div key={s.label} style={cardStyle}>
            <div style={statValue}>{s.value}</div>
            <div style={statLabel}>{s.label} (7 days)</div>
          </div>
        ))}
      </div>

      {/* Pending shares */}
      {data.pending_friend_shares > 0 && (
        <div style={{ ...cardStyle, marginBottom: "1.5rem", borderColor: "var(--color-primary)" }}>
          <p style={{ margin: 0 }}>
            You have <strong>{data.pending_friend_shares}</strong> pending share request{data.pending_friend_shares > 1 ? "s" : ""}.{" "}
            <Link to="/friends" style={{ color: "var(--color-primary)" }}>View</Link>
          </p>
        </div>
      )}

      {/* Latest lab */}
      {data.latest_lab_date && (
        <div style={cardStyle}>
          <p style={{ margin: 0, color: "var(--color-text-muted)", fontSize: "0.875rem" }}>
            Latest lab results: <strong style={{ color: "var(--color-text)" }}>{data.latest_lab_date}</strong>
          </p>
        </div>
      )}
    </main>
  );
}
