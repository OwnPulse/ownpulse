// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { exploreApi } from "../../api/explore";
import { interventionsApi } from "../../api/interventions";
import type { BeforeAfterResponse, MetricSpec } from "../../api/stats";
import { statsApi } from "../../api/stats";
import { BeforeAfterChart } from "./BeforeAfterChart";
import styles from "./Forms.module.css";
import { StatsCard } from "./StatsCard";

export function BeforeAfterForm() {
  const [substance, setSubstance] = useState("");
  const [metric, setMetric] = useState<MetricSpec | null>(null);
  const [beforeDays, setBeforeDays] = useState(30);
  const [afterDays, setAfterDays] = useState(30);
  const [resolution, setResolution] = useState<"daily" | "weekly" | "monthly">("daily");

  const metricsQuery = useQuery({
    queryKey: ["explore-metrics"],
    queryFn: exploreApi.getMetrics,
  });

  const substancesQuery = useQuery({
    queryKey: ["intervention-substances"],
    queryFn: async () => {
      const interventions = await interventionsApi.list();
      const unique = [...new Set(interventions.map((i) => i.substance))];
      unique.sort();
      return unique;
    },
  });

  const mutation = useMutation({
    mutationFn: (data: {
      substance: string;
      metric: MetricSpec;
      beforeDays: number;
      afterDays: number;
      resolution: "daily" | "weekly" | "monthly";
    }) =>
      statsApi.beforeAfter({
        intervention_substance: data.substance,
        metric: data.metric,
        before_days: data.beforeDays,
        after_days: data.afterDays,
        resolution: data.resolution,
      }),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!substance || !metric) return;
    mutation.mutate({ substance, metric, beforeDays, afterDays, resolution });
  };

  const sources = metricsQuery.data?.sources ?? [];
  const substances = substancesQuery.data ?? [];
  const result: BeforeAfterResponse | undefined = mutation.data;

  return (
    <div>
      <form onSubmit={handleSubmit} className={styles.form}>
        <div className={styles.row}>
          <div className="op-form-field">
            <label className="op-label" htmlFor="ba-substance">
              Substance
            </label>
            <input
              id="ba-substance"
              className="op-input"
              list="substance-options"
              value={substance}
              onChange={(e) => setSubstance(e.target.value)}
              placeholder="Type to search..."
            />
            <datalist id="substance-options">
              {substances.map((s) => (
                <option key={s} value={s} />
              ))}
            </datalist>
          </div>

          <div className="op-form-field">
            <label className="op-label" htmlFor="ba-metric">
              Metric
            </label>
            <select
              id="ba-metric"
              className="op-select"
              value={metric ? `${metric.source}:${metric.field}` : ""}
              onChange={(e) => {
                const [source, field] = e.target.value.split(":");
                if (source && field) setMetric({ source, field });
              }}
            >
              <option value="">Select a metric</option>
              {sources.map((group) =>
                group.metrics.map((m) => (
                  <option key={`${group.source}:${m.field}`} value={`${group.source}:${m.field}`}>
                    {group.label} - {m.label} ({m.unit})
                  </option>
                )),
              )}
            </select>
          </div>
        </div>

        <div className={styles.row}>
          <div className="op-form-field">
            <label className="op-label" htmlFor="ba-before-days">
              Before (days)
            </label>
            <input
              id="ba-before-days"
              className="op-input"
              type="number"
              min={1}
              max={365}
              value={beforeDays}
              onChange={(e) => setBeforeDays(Number(e.target.value))}
            />
          </div>

          <div className="op-form-field">
            <label className="op-label" htmlFor="ba-after-days">
              After (days)
            </label>
            <input
              id="ba-after-days"
              className="op-input"
              type="number"
              min={1}
              max={365}
              value={afterDays}
              onChange={(e) => setAfterDays(Number(e.target.value))}
            />
          </div>

          <div className="op-form-field">
            <label className="op-label" htmlFor="ba-resolution">
              Resolution
            </label>
            <select
              id="ba-resolution"
              className="op-select"
              value={resolution}
              onChange={(e) => setResolution(e.target.value as "daily" | "weekly" | "monthly")}
            >
              <option value="daily">Daily</option>
              <option value="weekly">Weekly</option>
              <option value="monthly">Monthly</option>
            </select>
          </div>
        </div>

        <button
          type="submit"
          className="op-btn op-btn-primary"
          disabled={!substance || !metric || mutation.isPending}
        >
          {mutation.isPending ? "Analyzing..." : "Analyze"}
        </button>
      </form>

      {mutation.isError && (
        <p className="op-error-msg" data-testid="ba-error">
          Error running analysis. Please try again.
        </p>
      )}

      {result && (
        <div data-testid="ba-results">
          <BeforeAfterChart
            before={result.before}
            after={result.after}
            firstDose={result.first_dose}
          />
          <StatsCard
            items={[
              { label: "Before Mean", value: result.before.mean.toFixed(2) },
              { label: "After Mean", value: result.after.mean.toFixed(2) },
              {
                label: "Change",
                value: `${result.change_pct >= 0 ? "+" : ""}${result.change_pct.toFixed(1)}%`,
              },
              {
                label: "p-value",
                value: result.p_value != null ? result.p_value.toFixed(4) : "N/A",
              },
              { label: "Test", value: result.test_used },
              { label: "Before N", value: String(result.before.n) },
              { label: "After N", value: String(result.after.n) },
            ]}
            significant={result.significant}
          />
          <p className={styles.disclaimer}>Correlation does not imply causation.</p>
        </div>
      )}
    </div>
  );
}
