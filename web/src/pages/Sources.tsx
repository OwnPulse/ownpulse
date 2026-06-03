// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { integrationsApi } from "../api/integrations";
import { Page } from "../components/ui/Page";
import { EmptyState, ErrorState, LoadingState } from "../components/ui/StateBlock";
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
    <Page title="Sources">
      <p className={styles.intro}>
        Connected sources feed OwnPulse with user-approved data. Apple Health sync is managed from
        the iOS app; server-side integrations appear here when configured.
      </p>

      {integrations.isLoading && <LoadingState label="Loading integrations..." />}
      {integrations.isError && <ErrorState message="Error loading integrations." />}
      {integrations.data && integrations.data.length === 0 && (
        <EmptyState>No integrations configured.</EmptyState>
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
    </Page>
  );
}
