// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery } from "@tanstack/react-query";
import { useNavigate, useParams } from "react-router-dom";
import { protocolsApi } from "../api/protocols";
import SequencerGrid from "../components/protocols/SequencerGrid";

export default function SharedProtocol() {
  const { token } = useParams<{ token: string }>();
  const navigate = useNavigate();

  const { data: protocol, isLoading, isError } = useQuery({
    queryKey: ["protocols", "shared", token],
    queryFn: () => protocolsApi.getShared(token!),
    enabled: !!token,
  });

  const importMutation = useMutation({
    mutationFn: () => protocolsApi.importProtocol(token!),
    onSuccess: (imported) => {
      navigate(`/protocols/${imported.id}`);
    },
  });

  if (isLoading) return <main className="op-page"><p>Loading...</p></main>;
  if (isError || !protocol) return <main className="op-page"><p>Protocol not found.</p></main>;

  return (
    <main className="op-page">
      <h1>{protocol.name}</h1>
      {protocol.description && <p>{protocol.description}</p>}
      <p style={{ fontSize: "var(--text-sm)", color: "var(--color-text-secondary)" }}>
        {protocol.duration_days} days &middot; {protocol.lines.length} substance
        {protocol.lines.length !== 1 ? "s" : ""}
      </p>

      <SequencerGrid
        lines={protocol.lines.map((l) => ({
          substance: l.substance,
          schedule_pattern: l.schedule_pattern,
        }))}
        durationDays={protocol.duration_days}
        editable={false}
      />

      <div style={{ marginTop: "1.5rem" }}>
        <button
          type="button"
          className="op-btn op-btn-primary"
          onClick={() => importMutation.mutate()}
          disabled={importMutation.isPending}
        >
          {importMutation.isPending ? "Importing..." : "Import Protocol"}
        </button>
        {importMutation.isError && (
          <p style={{ color: "var(--color-error)", fontSize: "var(--text-sm)", marginTop: "0.5rem" }}>
            Error: {importMutation.error.message}
          </p>
        )}
      </div>
    </main>
  );
}
