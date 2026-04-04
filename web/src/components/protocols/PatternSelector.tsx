// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

interface PatternSelectorProps {
  durationDays: number;
  onSelect: (pattern: boolean[]) => void;
}

type PatternName =
  | "Daily"
  | "Every Other Day"
  | "Twice a Week"
  | "3x per Week"
  | "Weekdays"
  | "Custom";

const PATTERNS: PatternName[] = [
  "Daily",
  "Every Other Day",
  "Twice a Week",
  "3x per Week",
  "Weekdays",
  "Custom",
];

export function generatePattern(name: PatternName, days: number): boolean[] | null {
  switch (name) {
    case "Daily":
      return Array(days).fill(true);
    case "Every Other Day":
      return Array.from({ length: days }, (_, i) => i % 2 === 0);
    case "Twice a Week": {
      // D1 and D4 of each 7-day cycle
      const tw = [true, false, false, true, false, false, false];
      return Array.from({ length: days }, (_, i) => tw[i % 7]);
    }
    case "3x per Week": {
      // D1, D3, D5 of each 7-day cycle
      const txw = [true, false, true, false, true, false, false];
      return Array.from({ length: days }, (_, i) => txw[i % 7]);
    }
    case "Weekdays": {
      // D1-D5 on, D6-D7 off
      const wd = [true, true, true, true, true, false, false];
      return Array.from({ length: days }, (_, i) => wd[i % 7]);
    }
    case "Custom":
      return null;
  }
}

export default function PatternSelector({ durationDays, onSelect }: PatternSelectorProps) {
  const handleChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const pattern = generatePattern(e.target.value as PatternName, durationDays);
    if (pattern) {
      onSelect(pattern);
    }
  };

  return (
    <select
      onChange={handleChange}
      defaultValue=""
      className="op-select"
      aria-label="Schedule pattern"
    >
      <option value="" disabled>
        Pattern...
      </option>
      {PATTERNS.map((p) => (
        <option key={p} value={p}>
          {p}
        </option>
      ))}
    </select>
  );
}
