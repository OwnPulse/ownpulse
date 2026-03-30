// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { SeriesResponse } from "../../api/explore";
import { metricKey, useExploreStore } from "../../stores/exploreStore";
import styles from "./ChartLegend.module.css";

const CHART_COLORS = [
  "#000000",
  "#E69F00",
  "#56B4E9",
  "#009E73",
  "#F0E442",
  "#0072B2",
  "#D55E00",
  "#CC79A7",
  "#332288",
  "#88CCEE",
  "#44AA99",
  "#DDCC77",
];

const SWATCH_PATTERNS = ["", styles.swatchPattern1, styles.swatchPattern2, styles.swatchPattern3];

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
            <span
              className={`${styles.swatch} ${SWATCH_PATTERNS[i % SWATCH_PATTERNS.length]}`}
              style={{ backgroundColor: color }}
            />
            <span className={styles.label}>
              {s.field} ({s.unit}){!hasData && <span className={styles.noData}> - no data</span>}
            </span>
          </button>
        );
      })}
    </div>
  );
}
