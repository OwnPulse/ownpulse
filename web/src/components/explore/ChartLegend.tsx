// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { SeriesResponse } from "../../api/explore";
import { metricKey, useExploreStore } from "../../stores/exploreStore";
import styles from "./ChartLegend.module.css";
import { LINE_DASH_PATTERNS } from "./ExploreChart";

const CHART_COLOR_VARS = [
  "var(--chart-color-0)",
  "var(--chart-color-1)",
  "var(--chart-color-2)",
  "var(--chart-color-3)",
  "var(--chart-color-4)",
  "var(--chart-color-5)",
  "var(--chart-color-6)",
  "var(--chart-color-7)",
  "#332288",
  "#88CCEE",
  "#44AA99",
  "#DDCC77",
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
        const color = CHART_COLOR_VARS[i % CHART_COLOR_VARS.length];
        const dashPattern = LINE_DASH_PATTERNS[i % LINE_DASH_PATTERNS.length];
        const hasData = s.points.length > 0;
        return (
          <button
            key={key}
            type="button"
            className={`${styles.item} ${hidden ? styles.hidden : ""}`}
            onClick={() => toggleVisibility(key)}
            aria-label={`Toggle ${s.field} visibility`}
          >
            <svg width="24" height="12" className={styles.legendLine} role="img" aria-hidden="true">
              <line
                x1="0"
                y1="6"
                x2="24"
                y2="6"
                stroke={color}
                strokeWidth={2.5}
                strokeDasharray={dashPattern ? dashPattern.join(",") : "none"}
              />
            </svg>
            <span className={styles.label}>
              {s.field} ({s.unit}){!hasData && <span className={styles.noData}> - no data</span>}
            </span>
          </button>
        );
      })}
    </div>
  );
}
