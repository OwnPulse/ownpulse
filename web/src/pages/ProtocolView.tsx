// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";
import { useParams } from "react-router-dom";
import { protocolsApi } from "../api/protocols";
import SequencerGrid from "../components/protocols/SequencerGrid";

export default function ProtocolView() {
  const { id } = useParams<{ id: string }>();

  const { data: protocol, isLoading, isError } = useQuery({
    queryKey: ["protocols", id],
    queryFn: () => protocolsApi.get(id!),
    enabled: !!id,
  });

  if (isLoading) return <main className="op-page"><p>Loading...</p></main>;
  if (isError || !protocol) return <main className="op-page"><p>Error loading protocol.</p></main>;

  const startDate = new Date(protocol.start_date);
  const today = new Date();
  const diffMs = today.getTime() - startDate.getTime();
  const todayIndex = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  return (
    <main className="op-page">
      <h1>{protocol.name}</h1>
      {protocol.description && <p>{protocol.description}</p>}
      <p style={{ fontSize: "var(--text-sm)", color: "var(--color-text-secondary)" }}>
        {protocol.status} &middot; {protocol.duration_days} days &middot; Started{" "}
        {protocol.start_date}
      </p>

      <SequencerGrid
        lines={protocol.lines.map((l) => ({
          substance: l.substance,
          schedule_pattern: l.schedule_pattern,
        }))}
        durationDays={protocol.duration_days}
        editable={false}
        todayIndex={todayIndex >= 0 && todayIndex < protocol.duration_days ? todayIndex : undefined}
      />
    </main>
  );
}
