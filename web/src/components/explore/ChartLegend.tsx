// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { SeriesResponse } from "../../api/explore";
import { metricKey, useExploreStore } from "../../stores/exploreStore";
import styles from "./ChartLegend.module.css";

const CHART_COLORS = [
  "#c2654a",
  "#3d8b8b",
  "#c49a3c",
  "#5a8a5a",
  "#9b59b6",
  "#1abc9c",
  "#f39c12",
  "#2980b9",
  "#d35400",
  "#27ae60",
  "#8e44ad",
  "#e74c3c",
];

interface ChartLegendProps {
  series: SeriesResponse[];
}

export function ChartLegend({ series }: ChartLegendProps) {
  const hiddenMetrics = useExploreStore((s) => s.hiddenMetrics);
  const toggleVisibility = useExploreStore((s) => s.toggleVisibility);

  if (series.length === 0) return null;

  return (
    <div className={styles.legend}>
      {series.map((s, i) => {
        const key = metricKey({ source: s.source, field: s.field });
        const hidden = hiddenMetrics.has(key);
        const color = CHART_COLORS[i % CHART_COLORS.length];
        const hasData = s.points.length > 0;
        return (
          <button
            key={key}
            type="button"
            className={`${styles.item} ${hidden ? styles.hidden : ""}`}
            onClick={() => toggleVisibility(key)}
            aria-label={`Toggle ${s.field} visibility`}
          >
            <span className={styles.swatch} style={{ backgroundColor: color }} />
            <span className={styles.label}>
              {s.field} ({s.unit}){!hasData && <span className={styles.noData}> - no data</span>}
            </span>
          </button>
        );
      })}
    </div>
  );
}
