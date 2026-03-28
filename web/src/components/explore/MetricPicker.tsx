// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { exploreApi } from "../../api/explore";
import { metricKey, useExploreStore } from "../../stores/exploreStore";
import styles from "./MetricPicker.module.css";

export function MetricPicker() {
  const [search, setSearch] = useState("");
  const selectedMetrics = useExploreStore((s) => s.selectedMetrics);
  const addMetric = useExploreStore((s) => s.addMetric);
  const removeMetric = useExploreStore((s) => s.removeMetric);

  const { data, isLoading, isError } = useQuery({
    queryKey: ["explore-metrics"],
    queryFn: exploreApi.getMetrics,
  });

  const selectedKeys = new Set(selectedMetrics.map(metricKey));
  const lowerSearch = search.toLowerCase();

  if (isLoading) return <div className={styles.picker}>Loading metrics...</div>;
  if (isError) return <div className={styles.picker}>Error loading metrics.</div>;

  const sources = data?.sources ?? [];

  return (
    <div className={styles.picker}>
      <input
        type="text"
        className="op-input"
        placeholder="Search metrics..."
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        aria-label="Search metrics"
      />
      <div className={styles.groups}>
        {sources.map((group) => {
          const filtered = group.metrics.filter(
            (m) =>
              !lowerSearch ||
              m.label.toLowerCase().includes(lowerSearch) ||
              m.field.toLowerCase().includes(lowerSearch),
          );
          if (filtered.length === 0) return null;
          return (
            <div key={group.source} className={styles.group}>
              <h3 className={styles.groupLabel}>{group.label}</h3>
              {filtered.map((m) => {
                const key = `${group.source}:${m.field}`;
                const checked = selectedKeys.has(key);
                return (
                  <label key={key} className={styles.metricItem}>
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() => {
                        if (checked) {
                          removeMetric({ source: group.source, field: m.field });
                        } else {
                          addMetric({ source: group.source, field: m.field });
                        }
                      }}
                    />
                    <span className={styles.metricLabel}>{m.label}</span>
                    <span className="op-badge op-badge-success">{m.unit}</span>
                  </label>
                );
              })}
            </div>
          );
        })}
      </div>
    </div>
  );
}
