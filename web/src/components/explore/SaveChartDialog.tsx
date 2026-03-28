// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type ChartConfig, exploreApi } from "../../api/explore";
import { useExploreStore } from "../../stores/exploreStore";
import styles from "./SaveChartDialog.module.css";

function toConfig(): ChartConfig {
  const state = useExploreStore.getState();
  const range =
    state.dateRange.type === "preset"
      ? { preset: state.dateRange.preset }
      : { start: state.dateRange.start, end: state.dateRange.end };
  return {
    version: 1,
    metrics: state.selectedMetrics.map((m) => ({ source: m.source, field: m.field })),
    range,
    resolution: state.resolution,
  };
}

interface SaveChartDialogProps {
  open: boolean;
  onClose: () => void;
}

export function SaveChartDialog({ open, onClose }: SaveChartDialogProps) {
  const [name, setName] = useState("");
  const queryClient = useQueryClient();

  const mutation = useMutation({
    mutationFn: (chartName: string) =>
      exploreApi.createChart({ name: chartName, config: toConfig() }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["explore-charts"] });
      setName("");
      onClose();
    },
  });

  if (!open) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (name.trim()) {
      mutation.mutate(name.trim());
    }
  };

  return (
    // biome-ignore lint/a11y/useKeyWithClickEvents: overlay dismiss is standard modal UX
    // biome-ignore lint/a11y/noStaticElementInteractions: overlay dismiss is standard modal UX
    <div className={styles.overlay} onClick={onClose}>
      {/* biome-ignore lint/a11y/noStaticElementInteractions: dialog stops propagation */}
      {/* biome-ignore lint/a11y/useKeyWithClickEvents: keyboard handled by Escape */}
      <div
        className={styles.dialog}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2>Save Chart</h2>
        <form onSubmit={handleSubmit}>
          <div className="op-form-field">
            <label className="op-label" htmlFor="chart-name">
              Chart Name
            </label>
            <input
              id="chart-name"
              className="op-input"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="My chart..."
              // biome-ignore lint/a11y/noAutofocus: dialog focus management
              autoFocus
            />
          </div>
          {mutation.isError && (
            <p className="op-error-msg">Failed to save chart. Please try again.</p>
          )}
          <div className={styles.actions}>
            <button type="button" className="op-btn op-btn-ghost" onClick={onClose}>
              Cancel
            </button>
            <button
              type="submit"
              className="op-btn op-btn-primary"
              disabled={!name.trim() || mutation.isPending}
            >
              {mutation.isPending ? "Saving..." : "Save"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
