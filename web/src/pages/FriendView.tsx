// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useParams, Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { friendsApi } from "../api/friends";

const cardStyle: React.CSSProperties = {
  background: "var(--color-surface)",
  borderRadius: "var(--radius-md)",
  border: "1px solid var(--color-border)",
  padding: "1.25rem",
  marginBottom: "1rem",
};

const tableStyle: React.CSSProperties = {
  width: "100%",
  borderCollapse: "collapse",
  fontSize: "0.8125rem",
};

const thStyle: React.CSSProperties = {
  textAlign: "left",
  padding: "0.375rem 0.5rem",
  borderBottom: "1px solid var(--color-border)",
  fontWeight: 600,
};

const tdStyle: React.CSSProperties = {
  padding: "0.375rem 0.5rem",
  borderBottom: "1px solid var(--color-border)",
};

interface CheckinRow {
  date?: string;
  energy?: number;
  mood?: number;
  focus?: number;
  stress?: number;
  sleep_quality?: number;
}

interface HealthRecordRow {
  record_type?: string;
  value?: number;
  unit?: string;
  source?: string;
  recorded_at?: string;
}

interface InterventionRow {
  substance?: string;
  dose?: number;
  unit?: string;
  taken_at?: string;
}

interface ObservationRow {
  type?: string;
  name?: string;
  recorded_at?: string;
}

interface LabResultRow {
  marker?: string;
  value?: number;
  unit?: string;
  collected_at?: string;
}

export default function FriendView() {
  const { friendId } = useParams<{ friendId: string }>();

  const { data, isLoading, isError } = useQuery({
    queryKey: ["friend-data", friendId],
    queryFn: () => friendsApi.getFriendData(friendId!),
    enabled: !!friendId,
  });

  if (isLoading) return <main style={{ padding: "1.5rem" }}>Loading...</main>;
  if (isError)
    return (
      <main style={{ padding: "1.5rem" }}>
        <p>Error loading friend data.</p>
        <Link to="/friends">Back to Friends</Link>
      </main>
    );

  const checkins = (data?.checkins ?? []) as CheckinRow[];
  const healthRecords = (data?.health_records ?? []) as HealthRecordRow[];
  const interventions = (data?.interventions ?? []) as InterventionRow[];
  const observations = (data?.observations ?? []) as ObservationRow[];
  const labResults = (data?.lab_results ?? []) as LabResultRow[];

  const hasData =
    checkins.length > 0 ||
    healthRecords.length > 0 ||
    interventions.length > 0 ||
    observations.length > 0 ||
    labResults.length > 0;

  return (
    <main style={{ padding: "1.5rem", maxWidth: "48rem", margin: "0 auto" }}>
      <div style={{ marginBottom: "1rem" }}>
        <Link to="/friends" style={{ fontSize: "0.875rem" }}>
          &larr; Back to Friends
        </Link>
      </div>
      <h1>Friend Data</h1>

      {!hasData && (
        <p style={{ color: "var(--color-text-muted)" }}>
          No shared data available.
        </p>
      )}

      {checkins.length > 0 && (
        <section style={cardStyle}>
          <h2 style={{ marginTop: 0 }}>Check-ins</h2>
          <table style={tableStyle}>
            <thead>
              <tr>
                <th style={thStyle}>Date</th>
                <th style={thStyle}>Energy</th>
                <th style={thStyle}>Mood</th>
                <th style={thStyle}>Focus</th>
                <th style={thStyle}>Stress</th>
                <th style={thStyle}>Sleep</th>
              </tr>
            </thead>
            <tbody>
              {checkins.map((c, i) => (
                <tr key={i}>
                  <td style={tdStyle}>{c.date ?? "-"}</td>
                  <td style={tdStyle}>{c.energy ?? "-"}</td>
                  <td style={tdStyle}>{c.mood ?? "-"}</td>
                  <td style={tdStyle}>{c.focus ?? "-"}</td>
                  <td style={tdStyle}>{c.stress ?? "-"}</td>
                  <td style={tdStyle}>{c.sleep_quality ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {healthRecords.length > 0 && (
        <section style={cardStyle}>
          <h2 style={{ marginTop: 0 }}>Health Records</h2>
          <table style={tableStyle}>
            <thead>
              <tr>
                <th style={thStyle}>Type</th>
                <th style={thStyle}>Value</th>
                <th style={thStyle}>Unit</th>
                <th style={thStyle}>Source</th>
                <th style={thStyle}>Time</th>
              </tr>
            </thead>
            <tbody>
              {healthRecords.map((r, i) => (
                <tr key={i}>
                  <td style={tdStyle}>{r.record_type ?? "-"}</td>
                  <td style={tdStyle}>{r.value ?? "-"}</td>
                  <td style={tdStyle}>{r.unit ?? "-"}</td>
                  <td style={tdStyle}>{r.source ?? "-"}</td>
                  <td style={tdStyle}>{r.recorded_at ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {interventions.length > 0 && (
        <section style={cardStyle}>
          <h2 style={{ marginTop: 0 }}>Interventions</h2>
          <table style={tableStyle}>
            <thead>
              <tr>
                <th style={thStyle}>Substance</th>
                <th style={thStyle}>Dose</th>
                <th style={thStyle}>Unit</th>
                <th style={thStyle}>Time</th>
              </tr>
            </thead>
            <tbody>
              {interventions.map((r, i) => (
                <tr key={i}>
                  <td style={tdStyle}>{r.substance ?? "-"}</td>
                  <td style={tdStyle}>{r.dose ?? "-"}</td>
                  <td style={tdStyle}>{r.unit ?? "-"}</td>
                  <td style={tdStyle}>{r.taken_at ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {observations.length > 0 && (
        <section style={cardStyle}>
          <h2 style={{ marginTop: 0 }}>Observations</h2>
          <table style={tableStyle}>
            <thead>
              <tr>
                <th style={thStyle}>Type</th>
                <th style={thStyle}>Name</th>
                <th style={thStyle}>Time</th>
              </tr>
            </thead>
            <tbody>
              {observations.map((r, i) => (
                <tr key={i}>
                  <td style={tdStyle}>{r.type ?? "-"}</td>
                  <td style={tdStyle}>{r.name ?? "-"}</td>
                  <td style={tdStyle}>{r.recorded_at ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {labResults.length > 0 && (
        <section style={cardStyle}>
          <h2 style={{ marginTop: 0 }}>Lab Results</h2>
          <table style={tableStyle}>
            <thead>
              <tr>
                <th style={thStyle}>Marker</th>
                <th style={thStyle}>Value</th>
                <th style={thStyle}>Unit</th>
                <th style={thStyle}>Date</th>
              </tr>
            </thead>
            <tbody>
              {labResults.map((r, i) => (
                <tr key={i}>
                  <td style={tdStyle}>{r.marker ?? "-"}</td>
                  <td style={tdStyle}>{r.value ?? "-"}</td>
                  <td style={tdStyle}>{r.unit ?? "-"}</td>
                  <td style={tdStyle}>{r.collected_at ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}
    </main>
  );
}
