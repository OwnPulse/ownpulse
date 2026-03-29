// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import { useSearchParams } from "react-router-dom";
import type { MetricSpec } from "../api/stats";
import { BeforeAfterForm } from "../components/analyze/BeforeAfterForm";
import { CorrelationForm } from "../components/analyze/CorrelationForm";
import { LagCorrelationForm } from "../components/analyze/LagCorrelationForm";
import styles from "./Analyze.module.css";

type AnalyzeMode = "before-after" | "correlation" | "lag";

function parseMetricParam(param: string | null): MetricSpec | undefined {
  if (!param) return undefined;
  const [source, field] = param.split(":");
  if (source && field) return { source, field };
  return undefined;
}

export default function Analyze() {
  const [searchParams] = useSearchParams();
  const [mode, setMode] = useState<AnalyzeMode>(() => {
    const m = searchParams.get("mode");
    if (m === "before-after" || m === "correlation" || m === "lag") return m;
    return "before-after";
  });

  const initialMetricA = parseMetricParam(searchParams.get("metricA"));
  const initialMetricB = parseMetricParam(searchParams.get("metricB"));

  const tabs: Array<{ key: AnalyzeMode; label: string }> = [
    { key: "before-after", label: "Before / After" },
    { key: "correlation", label: "Correlation" },
    { key: "lag", label: "Lag Correlation" },
  ];

  return (
    <main className="op-page">
      <div className="op-page-header">
        <h1>Analyze</h1>
      </div>

      <div className="op-tab-bar" role="tablist">
        {tabs.map((tab) => (
          <button
            key={tab.key}
            type="button"
            role="tab"
            className={`op-tab${mode === tab.key ? " active" : ""}`}
            aria-selected={mode === tab.key}
            onClick={() => setMode(tab.key)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      <div className={styles.content}>
        {mode === "before-after" && <BeforeAfterForm />}
        {mode === "correlation" && (
          <CorrelationForm initialMetricA={initialMetricA} initialMetricB={initialMetricB} />
        )}
        {mode === "lag" && (
          <LagCorrelationForm initialMetricA={initialMetricA} initialMetricB={initialMetricB} />
        )}
      </div>
    </main>
  );
}
