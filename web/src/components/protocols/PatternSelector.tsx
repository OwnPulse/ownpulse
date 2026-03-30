// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

interface PatternSelectorProps {
  durationDays: number;
  onSelect: (pattern: boolean[]) => void;
}

type PatternName = "Daily" | "MWF" | "Every Other Day" | "Weekdays" | "Custom";

const PATTERNS: PatternName[] = ["Daily", "MWF", "Every Other Day", "Weekdays", "Custom"];

function generatePattern(name: PatternName, days: number): boolean[] | null {
  switch (name) {
    case "Daily":
      return Array(days).fill(true);
    case "MWF": {
      // Mon, Wed, Fri = repeating [T, F, T, F, T, F, F]
      const mwf = [true, false, true, false, true, false, false];
      return Array.from({ length: days }, (_, i) => mwf[i % 7]);
    }
    case "Every Other Day":
      return Array.from({ length: days }, (_, i) => i % 2 === 0);
    case "Weekdays": {
      // Mon-Fri = repeating [T, T, T, T, T, F, F]
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
    <select onChange={handleChange} defaultValue="" className="op-select" aria-label="Schedule pattern">
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
