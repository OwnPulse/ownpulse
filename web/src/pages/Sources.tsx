// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { integrationsApi } from "../api/integrations";

export default function Sources() {
  const queryClient = useQueryClient();

  const integrations = useQuery({
    queryKey: ["integrations"],
    queryFn: () => integrationsApi.list(),
  });

  const disconnectMutation = useMutation({
    mutationFn: (source: string) => integrationsApi.disconnect(source),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["integrations"] });
    },
  });

  return (
    <main style={{ padding: "1.5rem" }}>
      <h1>Sources</h1>

      {integrations.isLoading && <p>Loading integrations...</p>}
      {integrations.isError && <p>Error loading integrations.</p>}
      {integrations.data && integrations.data.length === 0 && (
        <p>No integrations configured.</p>
      )}
      {integrations.data && (
        <ul style={{ listStyle: "none", padding: 0 }}>
          {integrations.data.map((integration) => (
            <li
              key={integration.source}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "1rem",
                padding: "0.75rem 0",
                borderBottom: "1px solid #eee",
              }}
            >
              <strong>{integration.source}</strong>
              <span
                style={{
                  color: integration.connected ? "green" : "gray",
                }}
              >
                {integration.connected ? "Connected" : "Disconnected"}
              </span>
              {integration.last_sync && (
                <span style={{ color: "#666", fontSize: "0.9em" }}>
                  Last sync: {integration.last_sync}
                </span>
              )}
              {integration.connected && (
                <button
                  onClick={() =>
                    disconnectMutation.mutate(integration.source)
                  }
                  disabled={disconnectMutation.isPending}
                >
                  Disconnect
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
    </main>
  );
}
