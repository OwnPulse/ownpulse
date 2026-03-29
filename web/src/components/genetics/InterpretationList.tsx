// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import type { Interpretation } from "../../api/genetics";
import { geneticsApi } from "../../api/genetics";
import { DisclaimerBanner } from "./DisclaimerBanner";
import { InterpretationCard } from "./InterpretationCard";
import styles from "./InterpretationList.module.css";

type Category = "all" | Interpretation["category"];

const CATEGORY_TABS: Array<{ key: Category; label: string }> = [
  { key: "all", label: "All" },
  { key: "health_risk", label: "Health Risks" },
  { key: "trait", label: "Traits" },
  { key: "pharmacogenomics", label: "Pharmacogenomics" },
  { key: "carrier_status", label: "Carrier Status" },
];

export function InterpretationList() {
  const [category, setCategory] = useState<Category>("all");

  const { data, isLoading, error } = useQuery({
    queryKey: ["genetics", "interpretations", category === "all" ? undefined : category],
    queryFn: () => geneticsApi.interpretations(category === "all" ? undefined : category),
  });

  return (
    <section className={styles.section}>
      <h2 className={styles.heading}>Interpretations</h2>

      <div className="op-tab-bar" role="tablist">
        {CATEGORY_TABS.map((tab) => (
          <button
            key={tab.key}
            type="button"
            role="tab"
            className={`op-tab${category === tab.key ? " active" : ""}`}
            aria-selected={category === tab.key}
            onClick={() => setCategory(tab.key)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {data?.disclaimer && <DisclaimerBanner text={data.disclaimer} />}
      {!data?.disclaimer && <DisclaimerBanner />}

      {isLoading && <p className="op-empty">Loading interpretations...</p>}

      {error && (
        <p className="op-error-msg">Failed to load interpretations: {(error as Error).message}</p>
      )}

      {data && data.interpretations.length === 0 && (
        <p className="op-empty">No interpretations available for this category.</p>
      )}

      {data?.interpretations.map((interp) => (
        <InterpretationCard key={interp.rsid} interpretation={interp} />
      ))}
    </section>
  );
}
