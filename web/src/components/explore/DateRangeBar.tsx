// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import { useExploreStore } from "../../stores/exploreStore";
import styles from "./DateRangeBar.module.css";

const PRESETS = ["7d", "30d", "90d", "1y", "all"] as const;

function formatDate(ts: number): string {
  const d = new Date(ts);
  return d.toISOString().slice(0, 10);
}

export function DateRangeBar() {
  const dateRange = useExploreStore((s) => s.dateRange);
  const setDateRange = useExploreStore((s) => s.setDateRange);
  const zoomRange = useExploreStore((s) => s.zoomRange);
  const resetZoom = useExploreStore((s) => s.resetZoom);
  const [showCustom, setShowCustom] = useState(false);
  const [customStart, setCustomStart] = useState("");
  const [customEnd, setCustomEnd] = useState("");

  const today = new Date().toISOString().slice(0, 10);

  const handlePreset = (preset: (typeof PRESETS)[number]) => {
    setShowCustom(false);
    setDateRange({ type: "preset", preset });
  };

  const handleCustomClick = () => {
    if (!showCustom && dateRange.type === "custom") {
      setCustomStart(dateRange.start);
      setCustomEnd(dateRange.end);
    }
    setShowCustom(!showCustom);
  };

  const handleCustomApply = () => {
    if (customStart && customEnd) {
      setDateRange({ type: "custom", start: customStart, end: customEnd });
    }
  };

  const activePreset = dateRange.type === "preset" ? dateRange.preset : null;

  return (
    <div className={styles.bar}>
      <div className={styles.presets}>
        {PRESETS.map((p) => (
          <button
            key={p}
            type="button"
            className={`op-btn op-btn-sm ${activePreset === p ? styles.active : "op-btn-ghost"}`}
            onClick={() => handlePreset(p)}
          >
            {p === "all" ? "All" : p.toUpperCase()}
          </button>
        ))}
        <button
          type="button"
          className={`op-btn op-btn-sm ${dateRange.type === "custom" ? styles.active : "op-btn-ghost"}`}
          onClick={handleCustomClick}
        >
          Custom
        </button>
        {zoomRange && (
          <button
            type="button"
            className="op-btn op-btn-sm op-btn-ghost"
            onClick={resetZoom}
            aria-label="Reset zoom"
          >
            Reset Zoom ({formatDate(zoomRange[0])} — {formatDate(zoomRange[1])})
          </button>
        )}
      </div>
      {showCustom && (
        <div className={styles.customRow}>
          <input
            type="date"
            className="op-input"
            value={customStart}
            max={today}
            onChange={(e) => setCustomStart(e.target.value)}
            aria-label="Start date"
          />
          <span className={styles.dateSep}>to</span>
          <input
            type="date"
            className="op-input"
            value={customEnd}
            max={today}
            onChange={(e) => setCustomEnd(e.target.value)}
            aria-label="End date"
          />
          <button
            type="button"
            className="op-btn op-btn-primary op-btn-sm"
            onClick={handleCustomApply}
            disabled={!customStart || !customEnd}
          >
            Apply
          </button>
        </div>
      )}
    </div>
  );
}
