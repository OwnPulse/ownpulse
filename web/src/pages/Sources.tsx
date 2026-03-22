// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { integrationsApi } from "../api/integrations";
import styles from "./Sources.module.css";

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
    <main className="op-page">
      <h1>Sources</h1>

      {integrations.isLoading && <p>Loading integrations...</p>}
      {integrations.isError && <p>Error loading integrations.</p>}
      {integrations.data && integrations.data.length === 0 && (
        <p className="op-empty">No integrations configured.</p>
      )}
      {integrations.data && (
        <ul className={styles.integrationList}>
          {integrations.data.map((integration) => (
            <li key={integration.source} className={styles.integrationItem}>
              <span className={styles.sourceName}>{integration.source}</span>
              <span
                className={
                  integration.connected ? styles.statusConnected : styles.statusDisconnected
                }
              >
                {integration.connected ? "Connected" : "Disconnected"}
              </span>
              {integration.last_sync && (
                <span className={styles.syncTime}>Last sync: {integration.last_sync}</span>
              )}
              {integration.connected && (
                <button
                  type="button"
                  className="op-btn op-btn-ghost op-btn-sm"
                  onClick={() => disconnectMutation.mutate(integration.source)}
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
