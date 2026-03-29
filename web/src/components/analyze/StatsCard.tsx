// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import styles from "./StatsCard.module.css";

interface StatsCardProps {
  items: Array<{ label: string; value: string }>;
  significant?: boolean;
}

export function StatsCard({ items, significant }: StatsCardProps) {
  return (
    <div className={`op-card ${styles.card}`}>
      <dl className={styles.grid}>
        {items.map((item) => (
          <div key={item.label} className={styles.item}>
            <dt className={styles.label}>{item.label}</dt>
            <dd className={styles.value}>{item.value}</dd>
          </div>
        ))}
      </dl>
      {significant !== undefined && (
        <p
          className={significant ? styles.significant : styles.notSignificant}
          data-testid="significance"
        >
          {significant
            ? "Statistically significant (p < 0.05)"
            : "Not statistically significant (p >= 0.05)"}
        </p>
      )}
    </div>
  );
}
