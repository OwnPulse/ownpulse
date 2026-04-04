// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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

/** Expand a 7-day template pattern to fill the target number of days. */
function expandTemplate(template: boolean[], targetDays: number): boolean[] {
  return Array.from({ length: targetDays }, (_, i) => template[i % 7]);
}

/** Extract the first 7 days from a full pattern as a template. */
function collapseToTemplate(pattern: boolean[]): boolean[] {
  return pattern.slice(0, 7);
}

const DRAFT_KEY = "protocol-builder-draft";

interface DraftState {
  name: string;
  weeks: number;
  lines: LineState[];
  templateMode?: boolean;
}

function loadDraft(): DraftState | null {
  try {
    const raw = sessionStorage.getItem(DRAFT_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as DraftState;
    // Basic shape validation
    if (
      typeof parsed.name !== "string" ||
      typeof parsed.weeks !== "number" ||
      !Array.isArray(parsed.lines)
    ) {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

function clearDraft() {
  sessionStorage.removeItem(DRAFT_KEY);
}

export default function ProtocolBuilder() {
  const navigate = useNavigate();
  const draft = useRef(loadDraft()).current;
  const [name, setName] = useState(draft?.name ?? "");
  const [description, setDescription] = useState("");
  const [weeks, setWeeks] = useState(draft?.weeks ?? 4);
  const [showCustomDuration, setShowCustomDuration] = useState(false);
  const [templateMode, setTemplateMode] = useState(draft?.templateMode ?? true);
  const durationDays = weeks * 7;
  const lineIdCounter = useRef(draft ? Math.max(...draft.lines.map((l) => l.id)) + 1 : 1);

  const [lines, setLines] = useState<LineState[]>(
    () => draft?.lines ?? [makeEmptyLine(0, templateMode ? 7 : durationDays)],
  );

  // Debounced save to sessionStorage
  const saveTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const saveDraft = useCallback(() => {
    if (saveTimerRef.current !== undefined) {
      clearTimeout(saveTimerRef.current);
    }
    saveTimerRef.current = setTimeout(() => {
      const state: DraftState = { name, weeks, lines, templateMode };
      sessionStorage.setItem(DRAFT_KEY, JSON.stringify(state));
    }, 300);
  }, [name, weeks, lines, templateMode]);

  useEffect(() => {
    saveDraft();
    return () => {
      if (saveTimerRef.current !== undefined) {
        clearTimeout(saveTimerRef.current);
      }
    };
  }, [saveDraft]);

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
      clearDraft();
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
    const patternLength = templateMode ? 7 : durationDays;
    setLines((prev) => [...prev, makeEmptyLine(id, patternLength)]);
  };

  const handleWeeksChange = (newWeeks: number) => {
    const newDays = templateMode ? 7 : newWeeks * 7;
    setWeeks(newWeeks);
    if (!templateMode) {
      setLines((prev) =>
        prev.map((line) => {
          const pattern = [...line.schedule_pattern];
          if (newDays > pattern.length) {
            while (pattern.length < newDays) {
              pattern.push(pattern[pattern.length % line.schedule_pattern.length] ?? true);
            }
          } else {
            pattern.length = newDays;
          }
          return { ...line, schedule_pattern: pattern };
        }),
      );
    }
    // In template mode, patterns stay at 7 days — weeks only affects the repeat count
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

  const handleSwitchMode = (toTemplate: boolean) => {
    if (toTemplate === templateMode) return;
    setTemplateMode(toTemplate);
    if (toTemplate) {
      // Full -> Template: collapse to first 7 days
      setLines((prev) =>
        prev.map((line) => ({
          ...line,
          schedule_pattern: collapseToTemplate(line.schedule_pattern),
        })),
      );
    } else {
      // Template -> Full: expand 7-day pattern to fill durationDays
      setLines((prev) =>
        prev.map((line) => ({
          ...line,
          schedule_pattern: expandTemplate(line.schedule_pattern, durationDays),
        })),
      );
    }
  };

  const handleCopyWeekForward = (weekIndex: number) => {
    setLines((prev) =>
      prev.map((line) => {
        const pattern = [...line.schedule_pattern];
        const srcStart = weekIndex * 7;
        const srcWeek = pattern.slice(srcStart, srcStart + 7);
        for (let w = weekIndex + 1; w * 7 < pattern.length; w++) {
          for (let d = 0; d < 7 && w * 7 + d < pattern.length; d++) {
            pattern[w * 7 + d] = srcWeek[d];
          }
        }
        return { ...line, schedule_pattern: pattern };
      }),
    );
  };

  const handleAddWeek = () => {
    const newWeeks = weeks + 1;
    setWeeks(newWeeks);
    setLines((prev) =>
      prev.map((line) => {
        const pattern = [...line.schedule_pattern];
        // Copy the last week's pattern
        const lastWeekStart = Math.max(0, pattern.length - 7);
        const lastWeek = pattern.slice(lastWeekStart, lastWeekStart + 7);
        return { ...line, schedule_pattern: [...pattern, ...lastWeek] };
      }),
    );
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const protocolLines: CreateProtocolLine[] = lines.map((line, i) => {
      // Always expand template to full pattern before sending to API
      const fullPattern = templateMode
        ? expandTemplate(line.schedule_pattern, durationDays)
        : line.schedule_pattern;

      return {
        substance: line.substance,
        dose: line.dose ? Number.parseFloat(line.dose) : undefined,
        unit: line.unit || undefined,
        route: line.route || undefined,
        time_of_day: line.time_of_day || undefined,
        schedule_pattern: fullPattern,
        sort_order: i,
      };
    });

    mutation.mutate({
      name,
      description: description || undefined,
      duration_days: durationDays,
      lines: protocolLines,
    });
  };

  const gridDays = templateMode ? 7 : durationDays;

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
                      const v = Number.parseInt(e.target.value, 10);
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

        {/* Mode toggle */}
        <fieldset className={styles.modeToggle} aria-label="Schedule mode">
          <button
            type="button"
            className={`op-btn op-btn-sm ${templateMode ? styles.modeActive : "op-btn-ghost"}`}
            onClick={() => handleSwitchMode(true)}
            aria-pressed={templateMode}
          >
            Week Template
          </button>
          <button
            type="button"
            className={`op-btn op-btn-sm ${!templateMode ? styles.modeActive : "op-btn-ghost"}`}
            onClick={() => handleSwitchMode(false)}
            aria-pressed={!templateMode}
          >
            Full Schedule
          </button>
        </fieldset>
        {templateMode && (
          <p className={styles.templateHint}>
            Edit one week — it repeats for {weeks === 1 ? "the full duration" : `${weeks} weeks`}.
          </p>
        )}

        {/* Lines section */}
        <div className={styles.linesSection}>
          <h2>Interventions</h2>
          {lines.map((line, idx) => (
            <div key={line.id} className={styles.lineCard}>
              {lines.length > 1 && (
                <button
                  type="button"
                  className={styles.removeBtn}
                  onClick={() => removeLine(idx)}
                  aria-label="Remove intervention"
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
                  durationDays={gridDays}
                  onSelect={(pattern) => handlePatternSelect(idx, pattern)}
                />
              </div>
            </div>
          ))}
          <button type="button" className="op-btn op-btn-ghost" onClick={addLine}>
            + Add Intervention
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
            durationDays={gridDays}
            editable
            onToggleCell={handleToggleCell}
            onCopyWeekForward={!templateMode ? handleCopyWeekForward : undefined}
          />
          {!templateMode && (
            <button
              type="button"
              className={`op-btn op-btn-ghost ${styles.addWeekBtn}`}
              onClick={handleAddWeek}
            >
              + Add Week
            </button>
          )}
        </div>

        {/* Actions */}
        <div className={styles.actions}>
          <button type="submit" disabled={mutation.isPending} className="op-btn op-btn-primary">
            {mutation.isPending ? "Creating..." : "Create Protocol"}
          </button>
          <Link to="/protocols" className="op-btn op-btn-ghost">
            Cancel
          </Link>
          <button
            type="button"
            className="op-btn op-btn-ghost"
            onClick={() => {
              clearDraft();
              setName("");
              setDescription("");
              setWeeks(4);
              setShowCustomDuration(false);
              setTemplateMode(true);
              lineIdCounter.current = 1;
              setLines([makeEmptyLine(0, 7)]);
            }}
          >
            Start Over
          </button>
        </div>
        {mutation.isError && <p className={forms.errorMsg}>Error: {mutation.error.message}</p>}
      </form>
    </main>
  );
}
