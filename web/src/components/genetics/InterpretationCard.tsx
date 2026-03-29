// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { Interpretation } from "../../api/genetics";
import styles from "./InterpretationCard.module.css";
import { RiskBadge } from "./RiskBadge";

function sourceUrl(source: string, sourceId: string | null): string | null {
  if (!sourceId) return null;
  if (source.toLowerCase() === "clinvar") {
    return `https://www.ncbi.nlm.nih.gov/clinvar/variation/${sourceId}/`;
  }
  if (source.toLowerCase() === "pharmgkb") {
    return `https://www.pharmgkb.org/variant/${sourceId}`;
  }
  return null;
}

export function InterpretationCard({ interpretation }: { interpretation: Interpretation }) {
  const { title, gene, chromosome, rsid, user_genotype, summary, risk_level } = interpretation;
  const { evidence_level, source, source_id, population_frequency } = interpretation;
  const url = sourceUrl(source, source_id);
  const mafLabel =
    population_frequency != null ? `MAF: ${(population_frequency * 100).toFixed(1)}%` : null;

  return (
    <div className={`op-card ${styles.card}`}>
      <div className={styles.header}>
        <h3 className={styles.title}>{title}</h3>
        <RiskBadge level={risk_level} />
      </div>

      <p className={styles.meta}>
        {gene && (
          <>
            Gene: {gene}
            <span className={styles.dot}> · </span>
          </>
        )}
        Chr {chromosome}
        <span className={styles.dot}> · </span>
        {rsid}
      </p>

      <p className={styles.genotype}>
        Your genotype: <strong>{user_genotype}</strong>
      </p>

      <p className={styles.summary}>{summary}</p>

      <p className={styles.evidence}>
        Evidence: {evidence_level}
        <span className={styles.dot}> · </span>
        Source:{" "}
        {url ? (
          <a href={url} target="_blank" rel="noopener noreferrer">
            {source}
          </a>
        ) : (
          source
        )}
        {mafLabel && (
          <>
            <span className={styles.dot}> · </span>
            {mafLabel}
          </>
        )}
      </p>
    </div>
  );
}
