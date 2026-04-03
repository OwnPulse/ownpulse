// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import styles from "./ScoreRing.module.css";

const SCORE_COLORS: Record<string, string> = {
  energy: "#c49a3c",
  mood: "#c2654a",
  focus: "#3d8b8b",
  recovery: "#5a8a5a",
  libido: "#7b61c2",
};

interface ScoreRingProps {
  label: string;
  value: number | null;
}

const RADIUS = 28;
const STROKE = 5;
const SIZE = (RADIUS + STROKE) * 2;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

export function ScoreRing({ label, value }: ScoreRingProps) {
  const color = SCORE_COLORS[label] ?? "#999";
  const progress = value != null ? value / 10 : 0;
  const offset = CIRCUMFERENCE * (1 - progress);
  const hasValue = value != null;

  return (
    <div className={styles.ring}>
      <svg
        width={SIZE}
        height={SIZE}
        viewBox={`0 0 ${SIZE} ${SIZE}`}
        className={styles.svg}
        role="img"
        aria-label={`${label} score: ${hasValue ? value : "none"} out of 10`}
      >
        {/* Background track */}
        <circle
          cx={SIZE / 2}
          cy={SIZE / 2}
          r={RADIUS}
          fill="none"
          stroke={color}
          strokeWidth={STROKE}
          opacity={0.15}
        />
        {/* Progress arc */}
        {hasValue && (
          <circle
            cx={SIZE / 2}
            cy={SIZE / 2}
            r={RADIUS}
            fill="none"
            stroke={color}
            strokeWidth={STROKE}
            strokeLinecap="round"
            strokeDasharray={CIRCUMFERENCE}
            strokeDashoffset={offset}
            className={styles.progress}
          />
        )}
      </svg>
      <span className={styles.value} style={hasValue ? { color } : undefined}>
        {hasValue ? value : "\u2014"}
      </span>
      <span className={styles.label}>{label}</span>
    </div>
  );
}
