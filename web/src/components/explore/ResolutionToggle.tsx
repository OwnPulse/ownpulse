// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { type Resolution, useExploreStore } from "../../stores/exploreStore";
import styles from "./ResolutionToggle.module.css";

const RESOLUTIONS: { value: Resolution; label: string }[] = [
  { value: "daily", label: "Daily" },
  { value: "weekly", label: "Weekly" },
  { value: "monthly", label: "Monthly" },
];

export function ResolutionToggle() {
  const resolution = useExploreStore((s) => s.resolution);
  const setResolution = useExploreStore((s) => s.setResolution);

  return (
    <fieldset className={styles.toggle} aria-label="Resolution">
      {RESOLUTIONS.map((r) => (
        <button
          key={r.value}
          type="button"
          className={`op-btn op-btn-sm ${resolution === r.value ? styles.active : "op-btn-ghost"}`}
          onClick={() => setResolution(r.value)}
          aria-pressed={resolution === r.value}
        >
          {r.label}
        </button>
      ))}
    </fieldset>
  );
}
