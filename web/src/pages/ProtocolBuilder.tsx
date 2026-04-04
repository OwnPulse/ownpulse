// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery } from "@tanstack/react-query";
import { useMemo, useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { interventionsApi } from "../api/interventions";
import { type CreateProtocol, type CreateProtocolLine, protocolsApi } from "../api/protocols";
import forms from "../components/forms/forms.module.css";
import PatternSelector from "../components/protocols/PatternSelector";
import SequencerGrid from "../components/protocols/SequencerGrid";
import styles from "./ProtocolBuilder.module.css";

const ROUTES = ["SubQ", "IM", "Oral", "Topical", "Nasal", "IV"] as const;

const DURATION_PRESETS = [
  { label: "2W", weeks: 2 },
  { label: "4W", weeks: 4 },
  { label: "8W", weeks: 8 },
  { label: "12W", weeks: 12 },
] as const;

const COMMON_SUBSTANCES = [
  "BPC-157",
  "TB-500",
  "GHK-Cu",
  "Sermorelin",
  "Ipamorelin",
  "CJC-1295",
  "MK-677",
  "Enclomiphene",
  "Testosterone Cypionate",
  "Metformin",
  "Rapamycin",
  "NAD+",
  "NMN",
  "Resveratrol",
  "Vitamin D3",
  "Vitamin K2",
  "Magnesium Glycinate",
  "Zinc",
  "Ashwagandha",
  "Tongkat Ali",
  "Creatine",
  "Fish Oil",
  "Melatonin",
  "L-Theanine",
  "Caffeine",
  "Modafinil",
];

interface LineState {
  id: number;
  substance: string;
  dose: string;
  unit: string;
  route: string;
  time_of_day: string;
  schedule_pattern: boolean[];
}

function todayDate() {
  return new Date().toISOString().slice(0, 10);
}

function makeEmptyLine(id: number, durationDays: number): LineState {
  return {
    id,
    substance: "",
    dose: "",
    unit: "",
    route: "",
    time_of_day: "",
    schedule_pattern: Array(durationDays).fill(true),
  };
}

export default function ProtocolBuilder() {
  const navigate = useNavigate();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [startDate, setStartDate] = useState(todayDate);
  const [weeks, setWeeks] = useState(4);
  const [showCustomDuration, setShowCustomDuration] = useState(false);
  const durationDays = weeks * 7;
  const lineIdCounter = useRef(1);

  const [lines, setLines] = useState<LineState[]>(() => [makeEmptyLine(0, durationDays)]);

  const { data: interventions } = useQuery({
    queryKey: ["interventions"],
    queryFn: () => interventionsApi.list(),
    staleTime: 5 * 60 * 1000,
  });

  const substanceSuggestions = useMemo(() => {
    const userSubstances = interventions
      ? [...new Set(interventions.map((iv) => iv.substance))]
      : [];
    const merged = [...new Set([...userSubstances, ...COMMON_SUBSTANCES])];
    merged.sort((a, b) => a.localeCompare(b));
    return merged;
  }, [interventions]);

  const mutation = useMutation({
    mutationFn: (data: CreateProtocol) => protocolsApi.create(data),
    onSuccess: (protocol) => {
      navigate(`/protocols/${protocol.id}`);
    },
  });

  const updateLine = (index: number, patch: Partial<LineState>) => {
    setLines((prev) => prev.map((l, i) => (i === index ? { ...l, ...patch } : l)));
  };

  const removeLine = (index: number) => {
    setLines((prev) => prev.filter((_, i) => i !== index));
  };

  const addLine = () => {
    const id = lineIdCounter.current++;
    setLines((prev) => [...prev, makeEmptyLine(id, durationDays)]);
  };

  const handleWeeksChange = (newWeeks: number) => {
    const newDays = newWeeks * 7;
    setWeeks(newWeeks);
    setLines((prev) =>
      prev.map((line) => {
        const pattern = [...line.schedule_pattern];
        if (newDays > pattern.length) {
          // Extend pattern by repeating the existing pattern
          while (pattern.length < newDays) {
            pattern.push(pattern[pattern.length % line.schedule_pattern.length] ?? true);
          }
        } else {
          pattern.length = newDays;
        }
        return { ...line, schedule_pattern: pattern };
      }),
    );
  };

  const handleToggleCell = (lineIndex: number, dayIndex: number) => {
    setLines((prev) =>
      prev.map((line, i) => {
        if (i !== lineIndex) return line;
        const pattern = [...line.schedule_pattern];
        pattern[dayIndex] = !pattern[dayIndex];
        return { ...line, schedule_pattern: pattern };
      }),
    );
  };

  const handlePatternSelect = (lineIndex: number, pattern: boolean[]) => {
    updateLine(lineIndex, { schedule_pattern: pattern });
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const protocolLines: CreateProtocolLine[] = lines.map((line, i) => ({
      substance: line.substance,
      dose: line.dose ? parseFloat(line.dose) : undefined,
      unit: line.unit || undefined,
      route: line.route || undefined,
      time_of_day: line.time_of_day || undefined,
      schedule_pattern: line.schedule_pattern,
      sort_order: i,
    }));

    mutation.mutate({
      name,
      description: description || undefined,
      start_date: startDate,
      duration_days: durationDays,
      lines: protocolLines,
    });
  };

  return (
    <main className="op-page">
      <h1>New Protocol</h1>
      <form onSubmit={handleSubmit}>
        {/* Protocol header */}
        <div className={styles.header}>
          <div className={styles.headerRow}>
            <div className={forms.field}>
              <label className={forms.label} htmlFor="proto-name">
                Name
              </label>
              <input
                id="proto-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                required
                className={forms.input}
                placeholder="e.g. BPC-157 + TB-500 Stack"
              />
            </div>
            <div className={forms.field}>
              <label className={forms.label} htmlFor="proto-start">
                Start Date
              </label>
              <input
                id="proto-start"
                type="date"
                value={startDate}
                onChange={(e) => setStartDate(e.target.value)}
                required
                className={forms.input}
              />
            </div>
            <div className={forms.field}>
              <span className={forms.label}>Duration</span>
              <div className={styles.durationPresets}>
                {DURATION_PRESETS.map((preset) => (
                  <button
                    key={preset.weeks}
                    type="button"
                    className={`op-btn op-btn-sm ${!showCustomDuration && weeks === preset.weeks ? styles.durationActive : "op-btn-ghost"}`}
                    onClick={() => {
                      setShowCustomDuration(false);
                      handleWeeksChange(preset.weeks);
                    }}
                  >
                    {preset.label}
                  </button>
                ))}
                <button
                  type="button"
                  className={`op-btn op-btn-sm ${showCustomDuration ? styles.durationActive : "op-btn-ghost"}`}
                  onClick={() => setShowCustomDuration(true)}
                >
                  Custom
                </button>
              </div>
              {showCustomDuration && (
                <div className={styles.customDuration}>
                  <input
                    id="proto-weeks"
                    type="number"
                    min={1}
                    max={52}
                    value={weeks}
                    onChange={(e) => {
                      const v = parseInt(e.target.value, 10);
                      if (v >= 1 && v <= 52) handleWeeksChange(v);
                    }}
                    className={forms.input}
                    aria-label="Custom duration in weeks"
                  />
                  <span className={styles.weeksSuffix}>{weeks === 1 ? "week" : "weeks"}</span>
                </div>
              )}
            </div>
          </div>
          <div className={forms.field}>
            <label className={forms.label} htmlFor="proto-desc">
              Description
            </label>
            <textarea
              id="proto-desc"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              className={forms.textarea}
              placeholder="Optional protocol notes..."
            />
          </div>
        </div>

        {/* Lines section */}
        <div className={styles.linesSection}>
          <h2>Lines</h2>
          {lines.map((line, idx) => (
            <div key={line.id} className={styles.lineCard}>
              {lines.length > 1 && (
                <button
                  type="button"
                  className={styles.removeBtn}
                  onClick={() => removeLine(idx)}
                  aria-label="Remove line"
                >
                  &times;
                </button>
              )}
              <div className={styles.lineFields}>
                <div className={forms.field}>
                  <label className={forms.label} htmlFor={`line-sub-${idx}`}>
                    Substance
                  </label>
                  <input
                    id={`line-sub-${idx}`}
                    list={`substance-suggestions-${idx}`}
                    value={line.substance}
                    onChange={(e) => updateLine(idx, { substance: e.target.value })}
                    required
                    className={forms.input}
                    autoComplete="off"
                  />
                  <datalist id={`substance-suggestions-${idx}`}>
                    {substanceSuggestions.map((s) => (
                      <option key={s} value={s} />
                    ))}
                  </datalist>
                </div>
                <div className={forms.field}>
                  <label className={forms.label} htmlFor={`line-dose-${idx}`}>
                    Dose
                  </label>
                  <input
                    id={`line-dose-${idx}`}
                    type="number"
                    step="any"
                    value={line.dose}
                    onChange={(e) => updateLine(idx, { dose: e.target.value })}
                    className={forms.input}
                  />
                </div>
                <div className={forms.field}>
                  <label className={forms.label} htmlFor={`line-unit-${idx}`}>
                    Unit
                  </label>
                  <input
                    id={`line-unit-${idx}`}
                    value={line.unit}
                    onChange={(e) => updateLine(idx, { unit: e.target.value })}
                    className={forms.input}
                    placeholder="mg, mcg, IU..."
                  />
                </div>
                <div className={forms.field}>
                  <label className={forms.label} htmlFor={`line-route-${idx}`}>
                    Route
                  </label>
                  <select
                    id={`line-route-${idx}`}
                    value={line.route}
                    onChange={(e) => updateLine(idx, { route: e.target.value })}
                    className={forms.select}
                  >
                    <option value="">Select...</option>
                    {ROUTES.map((r) => (
                      <option key={r} value={r}>
                        {r}
                      </option>
                    ))}
                  </select>
                </div>
                <div className={forms.field}>
                  <label className={forms.label} htmlFor={`line-time-${idx}`}>
                    Time
                  </label>
                  <select
                    id={`line-time-${idx}`}
                    value={line.time_of_day}
                    onChange={(e) => updateLine(idx, { time_of_day: e.target.value })}
                    className={forms.select}
                  >
                    <option value="">Any</option>
                    <option value="AM">AM</option>
                    <option value="PM">PM</option>
                  </select>
                </div>
              </div>
              <div className={styles.patternRow}>
                <span className={styles.patternLabel}>Schedule:</span>
                <PatternSelector
                  durationDays={durationDays}
                  onSelect={(pattern) => handlePatternSelect(idx, pattern)}
                />
              </div>
            </div>
          ))}
          <button type="button" className="op-btn op-btn-ghost" onClick={addLine}>
            + Add Line
          </button>
        </div>

        {/* Sequencer Grid */}
        <div className={styles.gridSection}>
          <h2>Schedule</h2>
          <SequencerGrid
            lines={lines.map((l) => ({
              substance: l.substance || "Untitled",
              schedule_pattern: l.schedule_pattern,
            }))}
            durationDays={durationDays}
            editable
            onToggleCell={handleToggleCell}
          />
        </div>

        {/* Actions */}
        <div className={styles.actions}>
          <button type="submit" disabled={mutation.isPending} className="op-btn op-btn-primary">
            {mutation.isPending ? "Creating..." : "Create Protocol"}
          </button>
          <Link to="/protocols" className="op-btn op-btn-ghost">
            Cancel
          </Link>
        </div>
        {mutation.isError && <p className={forms.errorMsg}>Error: {mutation.error.message}</p>}
      </form>
    </main>
  );
}
