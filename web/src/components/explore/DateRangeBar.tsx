// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import { useExploreStore } from "../../stores/exploreStore";
import styles from "./DateRangeBar.module.css";

const PRESETS = ["7d", "30d", "90d", "1y", "all"] as const;

export function DateRangeBar() {
  const dateRange = useExploreStore((s) => s.dateRange);
  const setDateRange = useExploreStore((s) => s.setDateRange);
  const [showCustom, setShowCustom] = useState(false);
  const [customStart, setCustomStart] = useState("");
  const [customEnd, setCustomEnd] = useState("");

  const handlePreset = (preset: (typeof PRESETS)[number]) => {
    setShowCustom(false);
    setDateRange({ type: "preset", preset });
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
          onClick={() => setShowCustom(!showCustom)}
        >
          Custom
        </button>
      </div>
      {showCustom && (
        <div className={styles.customRow}>
          <input
            type="date"
            className="op-input"
            value={customStart}
            onChange={(e) => setCustomStart(e.target.value)}
            aria-label="Start date"
          />
          <span className={styles.dateSep}>to</span>
          <input
            type="date"
            className="op-input"
            value={customEnd}
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
