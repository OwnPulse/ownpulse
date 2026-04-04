// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { SeriesResponse } from "../../api/explore";
import type { Intervention } from "../../api/interventions";
import { metricKey, useExploreStore } from "../../stores/exploreStore";
import styles from "./ChartLegend.module.css";
import { CHART_COLORS, INTERVENTION_COLOR } from "./chartColors";

interface ChartLegendProps {
  series: SeriesResponse[];
  interventions?: Intervention[];
}

export function ChartLegend({ series, interventions = [] }: ChartLegendProps) {
  const hiddenMetrics = useExploreStore((s) => s.hiddenMetrics);
  const toggleVisibility = useExploreStore((s) => s.toggleVisibility);
  const hiddenSubstances = useExploreStore((s) => s.hiddenSubstances);
  const toggleSubstanceVisibility = useExploreStore((s) => s.toggleSubstanceVisibility);

  if (series.length === 0 && interventions.length === 0) return null;

  const substanceCounts = new Map<string, number>();
  for (const iv of interventions) {
    substanceCounts.set(iv.substance, (substanceCounts.get(iv.substance) ?? 0) + 1);
  }
  const substances = [...substanceCounts.keys()];

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
              {s.field} ({s.unit})
              {hasData && <span className={styles.points}> ({s.points.length} pts)</span>}
              {!hasData && <span className={styles.noData}> - no data</span>}
            </span>
          </button>
        );
      })}
      {substances.map((sub) => {
        const hidden = hiddenSubstances.includes(sub);
        return (
          <button
            key={`iv-${sub}`}
            type="button"
            className={`${styles.item} ${hidden ? styles.hidden : ""}`}
            onClick={() => toggleSubstanceVisibility(sub)}
            aria-label={`Toggle ${sub} visibility`}
          >
            <span className={styles.swatch} style={{ backgroundColor: INTERVENTION_COLOR }} />
            <span className={styles.label}>
              {sub} ({substanceCounts.get(sub)})
            </span>
          </button>
        );
      })}
    </div>
  );
}
