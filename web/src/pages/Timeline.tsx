// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { checkinsApi } from "../api/checkins";
import { healthRecordsApi } from "../api/health-records";
import { sleepApi } from "../api/sleep";
import SleepChart from "../components/SleepChart";
import WeightChart from "../components/WeightChart";
import styles from "./Timeline.module.css";

export default function Timeline() {
  const healthRecords = useQuery({
    queryKey: ["health-records"],
    queryFn: () => healthRecordsApi.list(),
  });

  const checkins = useQuery({
    queryKey: ["checkins"],
    queryFn: () => checkinsApi.list(),
  });

  const sinceStr = useMemo(() => {
    const since = new Date();
    since.setDate(since.getDate() - 14);
    return since.toISOString().slice(0, 10);
  }, []);

  const weightSinceStr = useMemo(() => {
    const since = new Date();
    since.setDate(since.getDate() - 90);
    return since.toISOString();
  }, []);

  const weightRecords = useQuery({
    queryKey: ["weight", { since: weightSinceStr }],
    queryFn: () =>
      healthRecordsApi.list({ record_type: "body_mass", start: weightSinceStr }),
  });

  const sleepRecords = useQuery({
    queryKey: ["sleep", { since: sinceStr }],
    queryFn: () => sleepApi.list({ since: sinceStr }),
  });

  return (
    <main className="op-page">
      <h1>Timeline</h1>

      <section className={styles.section}>
        <h2>Sleep (Last 14 Days)</h2>
        {sleepRecords.isLoading && <p>Loading...</p>}
        {sleepRecords.isError && <p>Error loading sleep records.</p>}
        {sleepRecords.data && <SleepChart data={sleepRecords.data} />}
      </section>

      <section className={styles.section}>
        <h2>Weight (Last 90 Days)</h2>
        {weightRecords.isLoading && <p>Loading...</p>}
        {weightRecords.isError && <p>Error loading weight records.</p>}
        {weightRecords.data && <WeightChart data={weightRecords.data} />}
      </section>

      <section className={styles.section}>
        <h2>Recent Check-ins</h2>
        {checkins.isLoading && <p>Loading...</p>}
        {checkins.isError && <p>Error loading check-ins.</p>}
        {checkins.data && checkins.data.length === 0 && (
          <p className="op-empty">No check-ins yet.</p>
        )}
        {checkins.data && (
          <ul className={styles.list}>
            {checkins.data.map((c) => (
              <li key={c.id} className={styles.listItem}>
                <span className={styles.recordLabel}>{c.date}</span>
                <span className={styles.recordMeta}>
                  {" "}
                  &mdash; Energy: {c.energy}, Mood: {c.mood}, Focus: {c.focus}, Recovery:{" "}
                  {c.recovery}, Libido: {c.libido}
                  {c.notes && ` \u2014 ${c.notes}`}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className={styles.section}>
        <h2>Recent Health Records</h2>
        {healthRecords.isLoading && <p>Loading...</p>}
        {healthRecords.isError && <p>Error loading health records.</p>}
        {healthRecords.data && healthRecords.data.length === 0 && (
          <p className="op-empty">No health records yet.</p>
        )}
        {healthRecords.data && (
          <ul className={styles.list}>
            {healthRecords.data.map((r) => (
              <li key={r.id} className={styles.listItem}>
                <span className={styles.recordLabel}>{r.record_type}</span>
                <span className={styles.recordMeta}>
                  : {r.value} {r.unit} ({r.source}, {r.start_time})
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </main>
  );
}
