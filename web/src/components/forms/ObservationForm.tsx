// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type CreateObservation, observationsApi } from "../../api/observations";
import forms from "./forms.module.css";

const OBSERVATION_TYPES = [
  "event_instant",
  "event_duration",
  "scale",
  "symptom",
  "note",
  "context_tag",
  "environmental",
] as const;

function nowLocal() {
  return new Date().toISOString().slice(0, 16);
}

export default function ObservationForm() {
  const queryClient = useQueryClient();
  const [type, setType] = useState<string>("event_instant");
  const [name, setName] = useState("");
  const [startTime, setStartTime] = useState(nowLocal);
  const [endTime, setEndTime] = useState("");
  const [notesText, setNotesText] = useState("");
  const [numeric, setNumeric] = useState("");
  const [max, setMax] = useState("10");
  const [severity, setSeverity] = useState("1");
  const [unitVal, setUnitVal] = useState("");

  const mutation = useMutation({
    mutationFn: (data: CreateObservation) => observationsApi.create(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["observations"] });
      setName("");
      setStartTime(nowLocal());
      setEndTime("");
      setNotesText("");
      setNumeric("");
      setMax("10");
      setSeverity("1");
      setUnitVal("");
    },
  });

  const buildValue = (): Record<string, unknown> => {
    switch (type) {
      case "event_instant":
      case "context_tag":
        return notesText ? { notes: notesText } : {};
      case "event_duration":
        return notesText ? { notes: notesText } : {};
      case "scale":
        return { numeric: parseFloat(numeric), max: parseInt(max, 10) };
      case "symptom":
        return { severity: parseInt(severity, 10) };
      case "note":
        return { text: notesText };
      case "environmental":
        return {
          numeric: parseFloat(numeric),
          unit: unitVal,
        };
      default:
        return {};
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({
      type,
      name,
      start_time: startTime,
      end_time: endTime || undefined,
      value: buildValue(),
    });
  };

  return (
    <form onSubmit={handleSubmit} className={forms.form}>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="obs-type">
          Type
        </label>
        <select
          id="obs-type"
          value={type}
          onChange={(e) => setType(e.target.value)}
          className={forms.select}
        >
          {OBSERVATION_TYPES.map((t) => (
            <option key={t} value={t}>
              {t}
            </option>
          ))}
        </select>
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="obs-name">
          Name
        </label>
        <input
          id="obs-name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="obs-start">
          Start Time
        </label>
        <input
          id="obs-start"
          type="datetime-local"
          value={startTime}
          onChange={(e) => setStartTime(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      {type === "event_duration" && (
        <div className={forms.field}>
          <label className={forms.label} htmlFor="obs-end">
            End Time
          </label>
          <input
            id="obs-end"
            type="datetime-local"
            value={endTime}
            onChange={(e) => setEndTime(e.target.value)}
            className={forms.input}
          />
        </div>
      )}
      {(type === "scale" || type === "environmental") && (
        <div className={forms.field}>
          <label className={forms.label} htmlFor="obs-numeric">
            Numeric Value
          </label>
          <input
            id="obs-numeric"
            type="number"
            step="any"
            value={numeric}
            onChange={(e) => setNumeric(e.target.value)}
            required
            className={forms.input}
          />
        </div>
      )}
      {type === "scale" && (
        <div className={forms.field}>
          <label className={forms.label} htmlFor="obs-max">
            Max
          </label>
          <input
            id="obs-max"
            type="number"
            value={max}
            onChange={(e) => setMax(e.target.value)}
            required
            className={forms.input}
          />
        </div>
      )}
      {type === "symptom" && (
        <div className={forms.field}>
          <label className={forms.label} htmlFor="obs-severity">
            Severity (1-10)
          </label>
          <input
            id="obs-severity"
            type="number"
            min="1"
            max="10"
            value={severity}
            onChange={(e) => setSeverity(e.target.value)}
            required
            className={forms.input}
          />
        </div>
      )}
      {type === "environmental" && (
        <div className={forms.field}>
          <label className={forms.label} htmlFor="obs-unit">
            Unit
          </label>
          <input
            id="obs-unit"
            value={unitVal}
            onChange={(e) => setUnitVal(e.target.value)}
            required
            className={forms.input}
          />
        </div>
      )}
      {(type === "note" ||
        type === "event_instant" ||
        type === "event_duration" ||
        type === "context_tag") && (
        <div className={forms.field}>
          <label className={forms.label} htmlFor="obs-notes">
            {type === "note" ? "Text" : "Notes"}
          </label>
          <textarea
            id="obs-notes"
            value={notesText}
            onChange={(e) => setNotesText(e.target.value)}
            required={type === "note"}
            className={forms.textarea}
          />
        </div>
      )}
      <div className={forms.actions}>
        <button type="submit" disabled={mutation.isPending} className="op-btn op-btn-primary">
          {mutation.isPending ? "Saving..." : "Save Observation"}
        </button>
      </div>
      {mutation.isError && <p className={forms.errorMsg}>Error: {mutation.error.message}</p>}
      {mutation.isSuccess && <p className={forms.successMsg}>Saved!</p>}
    </form>
  );
}
