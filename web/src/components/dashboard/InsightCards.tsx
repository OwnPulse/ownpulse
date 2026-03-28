// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { insightsApi } from "../../api/insights";
import { InsightCard } from "./InsightCard";
import styles from "./InsightCards.module.css";

export function InsightCards() {
  const queryClient = useQueryClient();

  const {
    data: insights,
    isLoading,
    isError,
  } = useQuery({
    queryKey: ["insights"],
    queryFn: insightsApi.list,
  });

  const dismissMutation = useMutation({
    mutationFn: insightsApi.dismiss,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["insights"] });
    },
  });

  const generateMutation = useMutation({
    mutationFn: insightsApi.generate,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["insights"] });
    },
  });

  function handleDismiss(id: string) {
    dismissMutation.mutate(id);
  }

  function handleRefresh() {
    generateMutation.mutate();
  }

  return (
    <section className={styles.section} aria-label="Insights">
      <div className={styles.sectionHeader}>
        <h2 className={styles.sectionTitle}>Insights</h2>
        <button
          type="button"
          className={styles.refreshBtn}
          onClick={handleRefresh}
          disabled={generateMutation.isPending}
          aria-label="Refresh insights"
        >
          {generateMutation.isPending ? (
            <span className={styles.spinner} data-testid="refresh-spinner" />
          ) : (
            <span aria-hidden="true">&#x21bb;</span>
          )}
        </button>
      </div>

      {isLoading && <p className={styles.statusText}>Loading insights...</p>}

      {isError && <p className={styles.statusText}>Failed to load insights.</p>}

      {!isLoading && !isError && insights && insights.length === 0 && (
        <p className={styles.emptyText}>No insights right now. Check back later.</p>
      )}

      {insights && insights.length > 0 && (
        <div className={styles.cardList}>
          {insights.map((insight) => (
            <InsightCard key={insight.id} insight={insight} onDismiss={handleDismiss} />
          ))}
        </div>
      )}
    </section>
  );
}
