// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { exploreApi } from "../api/explore";
import { interventionsApi } from "../api/interventions";
import { ChartLegend } from "../components/explore/ChartLegend";
import { DateRangeBar } from "../components/explore/DateRangeBar";
import { ExploreChart } from "../components/explore/ExploreChart";
import { MetricPicker } from "../components/explore/MetricPicker";
import { ResolutionToggle } from "../components/explore/ResolutionToggle";
import { SaveChartDialog } from "../components/explore/SaveChartDialog";
import { SavedChartCard } from "../components/explore/SavedChartCard";
import { dateRangeToParams, useExploreStore } from "../stores/exploreStore";
import styles from "./Explore.module.css";

export default function Explore() {
  const { chartId } = useParams<{ chartId?: string }>();
  const navigate = useNavigate();
  const [saveOpen, setSaveOpen] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const queryClient = useQueryClient();

  const selectedMetrics = useExploreStore((s) => s.selectedMetrics);
  const dateRange = useExploreStore((s) => s.dateRange);
  const resolution = useExploreStore((s) => s.resolution);
  const loadConfig = useExploreStore((s) => s.loadConfig);

  // Load saved chart by URL param
  const savedChartQuery = useQuery({
    queryKey: ["explore-chart", chartId],
    queryFn: async () => {
      if (!chartId) throw new Error("chartId is required");
      const chart = await exploreApi.getChart(chartId);
      loadConfig(chart.config);
      return chart;
    },
    enabled: !!chartId,
  });

  // Fetch saved charts list
  const chartsQuery = useQuery({
    queryKey: ["explore-charts"],
    queryFn: exploreApi.listCharts,
  });

  // Fetch series data
  const { start, end } = dateRangeToParams(dateRange);
  const seriesQuery = useQuery({
    queryKey: ["explore-series", selectedMetrics, start, end, resolution],
    queryFn: () =>
      exploreApi.batchSeries({
        metrics: selectedMetrics.map((m) => ({ source: m.source, field: m.field })),
        start,
        end,
        resolution,
      }),
    enabled: selectedMetrics.length > 0,
  });

  // Fetch interventions for the visible range
  const interventionsQuery = useQuery({
    queryKey: ["explore-interventions", start, end],
    queryFn: () => interventionsApi.list({ start, end }),
    enabled: selectedMetrics.length > 0,
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => exploreApi.deleteChart(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["explore-charts"] });
    },
  });

  const seriesData = seriesQuery.data?.series ?? [];
  const interventionsData = interventionsQuery.data ?? [];

  return (
    <main className="op-page">
      <div className="op-page-header">
        <h1>Explore</h1>
        <div>
          {selectedMetrics.length >= 2 && (
            <button
              type="button"
              className="op-btn op-btn-secondary"
              onClick={() => {
                const params = new URLSearchParams({
                  mode: "correlation",
                  metricA: `${selectedMetrics[0].source}:${selectedMetrics[0].field}`,
                  metricB: `${selectedMetrics[1].source}:${selectedMetrics[1].field}`,
                });
                navigate(`/analyze?${params.toString()}`);
              }}
              style={{ marginRight: "0.5rem" }}
            >
              Correlate
            </button>
          )}
          {selectedMetrics.length > 0 && (
            <button
              type="button"
              className="op-btn op-btn-primary"
              onClick={() => setSaveOpen(true)}
            >
              Save Chart
            </button>
          )}
        </div>
      </div>

      <div className={styles.controls}>
        <DateRangeBar />
        <ResolutionToggle />
      </div>

      {/* Mobile picker toggle */}
      <button
        type="button"
        className={`op-btn op-btn-secondary ${styles.pickerToggle}`}
        onClick={() => setPickerOpen(!pickerOpen)}
      >
        {pickerOpen ? "Hide Metrics" : "Select Metrics"}
      </button>

      <div className={styles.layout}>
        <aside className={`${styles.sidebar} ${pickerOpen ? styles.sidebarOpen : ""}`}>
          <MetricPicker />
        </aside>
        <div className={styles.chartArea}>
          {seriesQuery.isLoading && selectedMetrics.length > 0 && <p>Loading chart data...</p>}
          {seriesQuery.isError && <p className="op-error-msg">Error loading chart data.</p>}
          <ExploreChart series={seriesData} interventions={interventionsData} />
          <ChartLegend series={seriesData} />
        </div>
      </div>

      {/* Saved charts */}
      {chartsQuery.data && chartsQuery.data.length > 0 && (
        <section className={styles.savedSection}>
          <h2>Saved Charts</h2>
          <div className={styles.savedRow}>
            {chartsQuery.data.map((chart) => (
              <SavedChartCard
                key={chart.id}
                chart={chart}
                onLoad={() => loadConfig(chart.config)}
                onDelete={() => deleteMutation.mutate(chart.id)}
              />
            ))}
          </div>
        </section>
      )}

      <SaveChartDialog open={saveOpen} onClose={() => setSaveOpen(false)} />

      {savedChartQuery.isLoading && chartId && <p>Loading saved chart...</p>}
    </main>
  );
}
