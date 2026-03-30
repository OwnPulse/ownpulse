// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import { Link } from "react-router-dom";
import type { Insight } from "../../api/insights";
import styles from "./InsightCards.module.css";

interface InsightCardProps {
  insight: Insight;
  onDismiss: (id: string) => void;
}

const TYPE_LABELS: Record<Insight["insight_type"], string> = {
  trend: "\uD83D\uDCC8 Trend",
  anomaly: "\u26A0\uFE0F Anomaly",
  missing_data: "\uD83D\uDCED Missing",
  streak: "\uD83D\uDD25 Streak",
  correlation: "\uD83D\uDD17 Correlation",
};

function exploreLink(insight: Insight): { to: string; label: string } | null {
  const params = insight.metadata.explore_params as
    | { source?: string; field?: string; preset?: string }
    | undefined;
  if (!params?.source || !params?.field) return null;

  const search = new URLSearchParams();
  search.set("source", params.source);
  search.set("field", params.field);
  if (params.preset) search.set("preset", params.preset);

  if (insight.insight_type === "correlation") {
    return { to: `/analyze?${search.toString()}`, label: "View in Analyze" };
  }
  return { to: `/explore?${search.toString()}`, label: "View in Explore" };
}

export function InsightCard({ insight, onDismiss }: InsightCardProps) {
  const [dismissed, setDismissed] = useState(false);
  const link = exploreLink(insight);

  function handleDismiss() {
    setDismissed(true);
    onDismiss(insight.id);
  }

  return (
    <div
      className={`${styles.card} ${styles[`border_${insight.insight_type}`]} ${dismissed ? styles.cardDismissed : ""}`}
      data-testid={`insight-card-${insight.id}`}
    >
      <div className={styles.cardContent}>
        <span className={`${styles.typeTag} ${styles[`tag_${insight.insight_type}`]}`}>
          {TYPE_LABELS[insight.insight_type]}
        </span>
        <p className={styles.headline}>{insight.headline}</p>
        {insight.detail && <p className={styles.detail}>{insight.detail}</p>}
        {link && (
          <Link to={link.to} className={styles.exploreLink}>
            {link.label} &rarr;
          </Link>
        )}
      </div>
      <button
        type="button"
        className={styles.dismissBtn}
        onClick={handleDismiss}
        aria-label={`Dismiss insight: ${insight.headline}`}
      >
        &times;
      </button>
    </div>
  );
}
