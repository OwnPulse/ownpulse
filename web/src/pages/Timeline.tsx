// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";
import { healthRecordsApi } from "../api/health-records";
import { checkinsApi } from "../api/checkins";
import { sleepApi } from "../api/sleep";
import SleepChart from "../components/SleepChart";

export default function Timeline() {
  const healthRecords = useQuery({
    queryKey: ["health-records"],
    queryFn: () => healthRecordsApi.list(),
  });

  const checkins = useQuery({
    queryKey: ["checkins"],
    queryFn: () => checkinsApi.list(),
  });

  const since = new Date();
  since.setDate(since.getDate() - 14);
  const sinceStr = since.toISOString().slice(0, 10);

  const sleepRecords = useQuery({
    queryKey: ["sleep", { since: sinceStr }],
    queryFn: () => sleepApi.list({ since: sinceStr }),
  });

  return (
    <main style={{ padding: "1.5rem" }}>
      <h1>Timeline</h1>

      <section>
        <h2>Sleep (Last 14 Days)</h2>
        {sleepRecords.isLoading && <p>Loading...</p>}
        {sleepRecords.isError && <p>Error loading sleep records.</p>}
        {sleepRecords.data && (
          <SleepChart data={sleepRecords.data} />
        )}
      </section>

      <section>
        <h2>Recent Check-ins</h2>
        {checkins.isLoading && <p>Loading...</p>}
        {checkins.isError && <p>Error loading check-ins.</p>}
        {checkins.data && checkins.data.length === 0 && (
          <p>No check-ins yet.</p>
        )}
        {checkins.data && (
          <ul>
            {checkins.data.map((c) => (
              <li key={c.id}>
                <strong>{c.date}</strong> — Energy: {c.energy}, Mood: {c.mood},
                Focus: {c.focus}, Recovery: {c.recovery}, Libido: {c.libido}
                {c.notes && ` — ${c.notes}`}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section>
        <h2>Recent Health Records</h2>
        {healthRecords.isLoading && <p>Loading...</p>}
        {healthRecords.isError && <p>Error loading health records.</p>}
        {healthRecords.data && healthRecords.data.length === 0 && (
          <p>No health records yet.</p>
        )}
        {healthRecords.data && (
          <ul>
            {healthRecords.data.map((r) => (
              <li key={r.id}>
                <strong>{r.record_type}</strong>: {r.value} {r.unit} (
                {r.source}, {r.start_time})
              </li>
            ))}
          </ul>
        )}
      </section>
    </main>
  );
}
