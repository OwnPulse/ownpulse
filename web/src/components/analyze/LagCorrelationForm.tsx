// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { exploreApi } from "../../api/explore";
import type { LagCorrelateResponse, MetricSpec } from "../../api/stats";
import { statsApi } from "../../api/stats";
import styles from "./Forms.module.css";
import { LagChart } from "./LagChart";
import { StatsCard } from "./StatsCard";

interface LagCorrelationFormProps {
  initialMetricA?: MetricSpec;
  initialMetricB?: MetricSpec;
}

export function LagCorrelationForm({ initialMetricA, initialMetricB }: LagCorrelationFormProps) {
  const [metricA, setMetricA] = useState<MetricSpec | null>(initialMetricA ?? null);
  const [metricB, setMetricB] = useState<MetricSpec | null>(initialMetricB ?? null);
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [maxLagDays, setMaxLagDays] = useState(7);
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
      maxLagDays: number;
      method: "pearson" | "spearman";
      resolution: "daily" | "weekly" | "monthly";
    }) =>
      statsApi.lagCorrelate({
        metric_a: data.metricA,
        metric_b: data.metricB,
        start: `${data.start}T00:00:00Z`,
        end: `${data.end}T23:59:59Z`,
        resolution: data.resolution,
        max_lag_days: data.maxLagDays,
        method: data.method,
      }),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!metricA || !metricB || !start || !end) return;
    mutation.mutate({ metricA, metricB, start, end, maxLagDays, method, resolution });
  };

  const sources = metricsQuery.data?.sources ?? [];
  const result: LagCorrelateResponse | undefined = mutation.data;

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
            <label className="op-label" htmlFor="lag-metric-a">
              Metric A
            </label>
            <select
              id="lag-metric-a"
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
            <label className="op-label" htmlFor="lag-metric-b">
              Metric B
            </label>
            <select
              id="lag-metric-b"
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
            <label className="op-label" htmlFor="lag-start">
              Start Date
            </label>
            <input
              id="lag-start"
              className="op-input"
              type="date"
              value={start}
              onChange={(e) => setStart(e.target.value)}
            />
          </div>

          <div className="op-form-field">
            <label className="op-label" htmlFor="lag-end">
              End Date
            </label>
            <input
              id="lag-end"
              className="op-input"
              type="date"
              value={end}
              onChange={(e) => setEnd(e.target.value)}
            />
          </div>

          <div className="op-form-field">
            <label className="op-label" htmlFor="lag-max-days">
              Max Lag (days)
            </label>
            <input
              id="lag-max-days"
              className="op-input"
              type="number"
              min={1}
              max={90}
              value={maxLagDays}
              onChange={(e) => setMaxLagDays(Number(e.target.value))}
            />
          </div>

          <div className="op-form-field">
            <label className="op-label" htmlFor="lag-resolution">
              Resolution
            </label>
            <select
              id="lag-resolution"
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
          {mutation.isPending ? "Analyzing..." : "Analyze Lag"}
        </button>
      </form>

      {mutation.isError && (
        <p className="op-error-msg" data-testid="lag-error">
          Error running lag analysis. Please try again.
        </p>
      )}

      {result && (
        <div data-testid="lag-results">
          <LagChart lags={result.lags} bestLag={result.best_lag.lag} />
          <StatsCard
            items={[
              { label: "Best Lag", value: `${result.best_lag.lag} days` },
              { label: "Best r", value: result.best_lag.r.toFixed(4) },
              { label: "Best p-value", value: result.best_lag.p_value.toFixed(4) },
              { label: "Best N", value: String(result.best_lag.n) },
              { label: "Method", value: result.method },
              { label: "Lags Tested", value: String(result.lags.length) },
            ]}
          />
          <p className={styles.disclaimer}>Correlation does not imply causation.</p>
        </div>
      )}
    </div>
  );
}
