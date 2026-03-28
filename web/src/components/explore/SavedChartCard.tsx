// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { SavedChart } from "../../api/explore";
import styles from "./SavedChartCard.module.css";

interface SavedChartCardProps {
  chart: SavedChart;
  onLoad: () => void;
  onDelete: () => void;
}

export function SavedChartCard({ chart, onLoad, onDelete }: SavedChartCardProps) {
  const metricCount = chart.config.metrics.length;
  const rangeLabel =
    "preset" in chart.config.range
      ? chart.config.range.preset.toUpperCase()
      : `${chart.config.range.start} - ${chart.config.range.end}`;

  const handleDelete = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (window.confirm(`Delete chart "${chart.name}"?`)) {
      onDelete();
    }
  };

  return (
    <div
      className={styles.card}
      onClick={onLoad}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onLoad();
        }
      }}
    >
      <div className={styles.name}>{chart.name}</div>
      <div className={styles.meta}>
        {metricCount} metric{metricCount !== 1 ? "s" : ""} &middot; {rangeLabel} &middot;{" "}
        {chart.config.resolution}
      </div>
      <button
        type="button"
        className={`op-btn op-btn-sm op-btn-danger ${styles.deleteBtn}`}
        onClick={handleDelete}
        aria-label={`Delete chart ${chart.name}`}
      >
        Delete
      </button>
    </div>
  );
}
