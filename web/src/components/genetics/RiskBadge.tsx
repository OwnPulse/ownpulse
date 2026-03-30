// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import type { Interpretation } from "../../api/genetics";
import styles from "./RiskBadge.module.css";

type RiskLevel = Interpretation["risk_level"];

const RISK_LABELS: Record<RiskLevel, string> = {
  high: "\u26A0\uFE0F High",
  moderate: "\u26A1 Moderate",
  low: "\u2713 Low",
  normal: "\u2713 Normal",
  poor_metabolizer: "\u26A0\uFE0F Poor Metabolizer",
  intermediate: "\u26A1 Intermediate",
  rapid: "\uD83D\uDD04 Rapid",
};

function riskColorClass(level: RiskLevel): string {
  switch (level) {
    case "high":
    case "poor_metabolizer":
      return styles.high;
    case "moderate":
    case "intermediate":
      return styles.moderate;
    case "low":
    case "normal":
      return styles.low;
    case "rapid":
      return styles.rapid;
  }
}

export function RiskBadge({ level }: { level: RiskLevel }) {
  return <span className={`${styles.badge} ${riskColorClass(level)}`}>{RISK_LABELS[level]}</span>;
}
