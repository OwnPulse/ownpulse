// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { exploreApi } from "../../api/explore";
import type { CorrelateResponse, MetricSpec } from "../../api/stats";
import { statsApi } from "../../api/stats";
import styles from "./Forms.module.css";
import { ScatterChart } from "./ScatterChart";
import { StatsCard } from "./StatsCard";

interface CorrelationFormProps {
  initialMetricA?: MetricSpec;
  initialMetricB?: MetricSpec;
}

export function CorrelationForm({ initialMetricA, initialMetricB }: CorrelationFormProps) {
  const [metricA, setMetricA] = useState<MetricSpec | null>(initialMetricA ?? null);
  const [metricB, setMetricB] = useState<MetricSpec | null>(initialMetricB ?? null);
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [method, setMethod] = useState<"pearson" | "spearman">("pearson");
  const [resolution, setResolution] = useState<"daily" | "weekly" | "monthly">("daily");

  const metricsQuery = useQuery({
    queryKey: ["explore-metrics"],
    queryFn: exploreApi.getMetrics,
  });

  const mutation = useMutation({
    mutationFn: (data: {
      metricA: MetricSpec;
      metricB: MetricSpec;
      start: string;
      end: string;
      method: "pearson" | "spearman";
      resolution: "daily" | "weekly" | "monthly";
    }) =>
      statsApi.correlate({
        metric_a: data.metricA,
        metric_b: data.metricB,
        start: `${data.start}T00:00:00Z`,
        end: `${data.end}T23:59:59Z`,
        resolution: data.resolution,
        method: data.method,
      }),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!metricA || !metricB || !start || !end) return;
    mutation.mutate({ metricA, metricB, start, end, method, resolution });
  };

  const sources = metricsQuery.data?.sources ?? [];
  const result: CorrelateResponse | undefined = mutation.data;

  const parseMetric = (value: string): MetricSpec | null => {
    const [source, field] = value.split(":");
    if (source && field) return { source, field };
    return null;
  };

  const metricOptions = sources.flatMap((group) =>
    group.metrics.map((m) => ({
      value: `${group.source}:${m.field}`,
      label: `${group.label} - ${m.label} (${m.unit})`,
    })),
  );

  return (
    <div>
      <form onSubmit={handleSubmit} className={styles.form}>
        <div className={styles.row}>
          <div className="op-form-field">
            <label className="op-label" htmlFor="corr-metric-a">
              Metric A
            </label>
            <select
              id="corr-metric-a"
              className="op-select"
              value={metricA ? `${metricA.source}:${metricA.field}` : ""}
              onChange={(e) => setMetricA(parseMetric(e.target.value))}
            >
              <option value="">Select metric A</option>
              {metricOptions.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </div>

          <div className="op-form-field">
            <label className="op-label" htmlFor="corr-metric-b">
              Metric B
            </label>
            <select
              id="corr-metric-b"
              className="op-select"
              value={metricB ? `${metricB.source}:${metricB.field}` : ""}
              onChange={(e) => setMetricB(parseMetric(e.target.value))}
            >
              <option value="">Select metric B</option>
              {metricOptions.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className={styles.row}>
          <div className="op-form-field">
            <label className="op-label" htmlFor="corr-start">
              Start Date
            </label>
            <input
              id="corr-start"
              className="op-input"
              type="date"
              value={start}
              onChange={(e) => setStart(e.target.value)}
            />
          </div>

          <div className="op-form-field">
            <label className="op-label" htmlFor="corr-end">
              End Date
            </label>
            <input
              id="corr-end"
              className="op-input"
              type="date"
              value={end}
              onChange={(e) => setEnd(e.target.value)}
            />
          </div>

          <div className="op-form-field">
            <label className="op-label" htmlFor="corr-resolution">
              Resolution
            </label>
            <select
              id="corr-resolution"
              className="op-select"
              value={resolution}
              onChange={(e) => setResolution(e.target.value as "daily" | "weekly" | "monthly")}
            >
              <option value="daily">Daily</option>
              <option value="weekly">Weekly</option>
              <option value="monthly">Monthly</option>
            </select>
          </div>

          <div className="op-form-field">
            <span className="op-label">Method</span>
            <div className={styles.toggleGroup}>
              <button
                type="button"
                className={`op-btn op-btn-sm ${method === "pearson" ? "op-btn-primary" : "op-btn-ghost"}`}
                onClick={() => setMethod("pearson")}
                aria-pressed={method === "pearson"}
              >
                Pearson
              </button>
              <button
                type="button"
                className={`op-btn op-btn-sm ${method === "spearman" ? "op-btn-primary" : "op-btn-ghost"}`}
                onClick={() => setMethod("spearman")}
                aria-pressed={method === "spearman"}
              >
                Spearman
              </button>
            </div>
          </div>
        </div>

        <button
          type="submit"
          className="op-btn op-btn-primary"
          disabled={!metricA || !metricB || !start || !end || mutation.isPending}
        >
          {mutation.isPending ? "Analyzing..." : "Correlate"}
        </button>
      </form>

      {mutation.isError && (
        <p className="op-error-msg" data-testid="corr-error">
          Error running correlation. Please try again.
        </p>
      )}

      {result && (
        <div data-testid="corr-results">
          <ScatterChart
            data={result.scatter}
            labelA={`${result.metric_a.source}:${result.metric_a.field}`}
            labelB={`${result.metric_b.source}:${result.metric_b.field}`}
          />
          <StatsCard
            items={[
              { label: "r", value: result.r.toFixed(4) },
              { label: "p-value", value: result.p_value.toFixed(4) },
              { label: "N", value: String(result.n) },
              { label: "Method", value: result.method },
              { label: "Interpretation", value: result.interpretation },
            ]}
            significant={result.significant}
          />
          <p className={styles.disclaimer}>Correlation does not imply causation.</p>
        </div>
      )}
    </div>
  );
}
