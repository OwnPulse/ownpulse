// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { protocolsApi } from "../api/protocols";

export default function Protocols() {
  const { data, isLoading, isError } = useQuery({
    queryKey: ["protocols"],
    queryFn: () => protocolsApi.list(),
  });

  return (
    <main className="op-page">
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <h1>Protocols</h1>
        <Link to="/protocols/new" className="op-btn op-btn-primary">
          New Protocol
        </Link>
      </div>

      {isLoading && <p>Loading...</p>}
      {isError && <p>Error loading protocols.</p>}

      {data && data.length === 0 && <p>No protocols yet. Create one to get started.</p>}

      {data?.map((protocol) => (
        <Link
          key={protocol.id}
          to={`/protocols/${protocol.id}`}
          className="op-card"
          style={{ display: "block", marginBottom: "0.75rem", textDecoration: "none" }}
        >
          <strong>{protocol.name}</strong>
          <div style={{ fontSize: "var(--text-sm)", color: "var(--color-text-secondary)" }}>
            {protocol.status} &middot; {protocol.progress_pct}% complete &middot;{" "}
            {protocol.duration_days} days
          </div>
        </Link>
      ))}
    </main>
  );
}
