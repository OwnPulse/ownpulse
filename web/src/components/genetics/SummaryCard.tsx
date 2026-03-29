// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { GeneticSummary } from "../../api/genetics";
import styles from "./SummaryCard.module.css";

export function SummaryCard({ summary }: { summary: GeneticSummary }) {
  const uploadDate = summary.uploaded_at
    ? new Date(summary.uploaded_at).toLocaleDateString()
    : "Unknown";

  const chromosomeEntries = Object.entries(summary.chromosomes).sort(([a], [b]) => {
    const numA = Number(a);
    const numB = Number(b);
    if (!Number.isNaN(numA) && !Number.isNaN(numB)) return numA - numB;
    if (!Number.isNaN(numA)) return -1;
    if (!Number.isNaN(numB)) return 1;
    return a.localeCompare(b);
  });

  return (
    <div className={`op-card ${styles.card}`}>
      <h3 className={styles.heading}>Genetic Data Summary</h3>

      <dl className={styles.stats}>
        <div className={styles.stat}>
          <dt>Total Variants</dt>
          <dd>{summary.total_variants.toLocaleString()}</dd>
        </div>
        <div className={styles.stat}>
          <dt>Source</dt>
          <dd>{summary.source ?? "Unknown"}</dd>
        </div>
        <div className={styles.stat}>
          <dt>Uploaded</dt>
          <dd>{uploadDate}</dd>
        </div>
        <div className={styles.stat}>
          <dt>Annotated</dt>
          <dd>{summary.annotated_count.toLocaleString()}</dd>
        </div>
      </dl>

      {chromosomeEntries.length > 0 && (
        <div className={styles.chromosomes}>
          <h4 className={styles.subheading}>Chromosome Distribution</h4>
          <div className={styles.chromGrid}>
            {chromosomeEntries.map(([chr, count]) => (
              <span key={chr} className={styles.chromItem}>
                <strong>Chr {chr}:</strong> {count.toLocaleString()}
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
