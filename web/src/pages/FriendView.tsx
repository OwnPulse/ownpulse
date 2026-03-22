// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";
import { Link, useParams } from "react-router-dom";
import { friendsApi } from "../api/friends";
import styles from "./FriendView.module.css";

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
    queryFn: () => friendsApi.getFriendData(friendId as string),
    enabled: !!friendId,
  });

  if (isLoading) return <main className="op-page">Loading...</main>;
  if (isError)
    return (
      <main className="op-page">
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
    <main className={`op-page ${styles.page}`}>
      <div className={styles.backRow}>
        <Link to="/friends" className={styles.backLink}>
          &larr; Back to Friends
        </Link>
      </div>
      <h1>Friend Data</h1>

      {!hasData && <p className="op-empty">No shared data available.</p>}

      {checkins.length > 0 && (
        <section className={`op-card ${styles.section}`}>
          <h2 className={styles.sectionTitle}>Check-ins</h2>
          <table className="op-table">
            <thead>
              <tr>
                <th>Date</th>
                <th>Energy</th>
                <th>Mood</th>
                <th>Focus</th>
                <th>Stress</th>
                <th>Sleep</th>
              </tr>
            </thead>
            <tbody>
              {checkins.map((c) => (
                <tr key={c.date ?? crypto.randomUUID()}>
                  <td>{c.date ?? "-"}</td>
                  <td>{c.energy ?? "-"}</td>
                  <td>{c.mood ?? "-"}</td>
                  <td>{c.focus ?? "-"}</td>
                  <td>{c.stress ?? "-"}</td>
                  <td>{c.sleep_quality ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {healthRecords.length > 0 && (
        <section className={`op-card ${styles.section}`}>
          <h2 className={styles.sectionTitle}>Health Records</h2>
          <table className="op-table">
            <thead>
              <tr>
                <th>Type</th>
                <th>Value</th>
                <th>Unit</th>
                <th>Source</th>
                <th>Time</th>
              </tr>
            </thead>
            <tbody>
              {healthRecords.map((r) => (
                <tr key={`${r.record_type}-${r.recorded_at}`}>
                  <td>{r.record_type ?? "-"}</td>
                  <td>{r.value ?? "-"}</td>
                  <td>{r.unit ?? "-"}</td>
                  <td>{r.source ?? "-"}</td>
                  <td>{r.recorded_at ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {interventions.length > 0 && (
        <section className={`op-card ${styles.section}`}>
          <h2 className={styles.sectionTitle}>Interventions</h2>
          <table className="op-table">
            <thead>
              <tr>
                <th>Substance</th>
                <th>Dose</th>
                <th>Unit</th>
                <th>Time</th>
              </tr>
            </thead>
            <tbody>
              {interventions.map((r) => (
                <tr key={`${r.substance}-${r.taken_at}`}>
                  <td>{r.substance ?? "-"}</td>
                  <td>{r.dose ?? "-"}</td>
                  <td>{r.unit ?? "-"}</td>
                  <td>{r.taken_at ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {observations.length > 0 && (
        <section className={`op-card ${styles.section}`}>
          <h2 className={styles.sectionTitle}>Observations</h2>
          <table className="op-table">
            <thead>
              <tr>
                <th>Type</th>
                <th>Name</th>
                <th>Time</th>
              </tr>
            </thead>
            <tbody>
              {observations.map((r) => (
                <tr key={`${r.type}-${r.name}-${r.recorded_at}`}>
                  <td>{r.type ?? "-"}</td>
                  <td>{r.name ?? "-"}</td>
                  <td>{r.recorded_at ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {labResults.length > 0 && (
        <section className={`op-card ${styles.section}`}>
          <h2 className={styles.sectionTitle}>Lab Results</h2>
          <table className="op-table">
            <thead>
              <tr>
                <th>Marker</th>
                <th>Value</th>
                <th>Unit</th>
                <th>Date</th>
              </tr>
            </thead>
            <tbody>
              {labResults.map((r) => (
                <tr key={`${r.marker}-${r.collected_at}`}>
                  <td>{r.marker ?? "-"}</td>
                  <td>{r.value ?? "-"}</td>
                  <td>{r.unit ?? "-"}</td>
                  <td>{r.collected_at ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}
    </main>
  );
}
