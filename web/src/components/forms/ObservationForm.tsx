// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type CreateObservation, observationsApi } from "../../api/observations";

const OBSERVATION_TYPES = [
  "event_instant",
  "event_duration",
  "scale",
  "symptom",
  "note",
  "context_tag",
  "environmental",
] as const;

export default function ObservationForm() {
  const queryClient = useQueryClient();
  const [type, setType] = useState<string>("event_instant");
  const [name, setName] = useState("");
  const [startTime, setStartTime] = useState("");
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
      setStartTime("");
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
    <form onSubmit={handleSubmit}>
      <div>
        <label>
          Type
          <select value={type} onChange={(e) => setType(e.target.value)}>
            {OBSERVATION_TYPES.map((t) => (
              <option key={t} value={t}>
                {t}
              </option>
            ))}
          </select>
        </label>
      </div>
      <div>
        <label>
          Name
          <input value={name} onChange={(e) => setName(e.target.value)} required />
        </label>
      </div>
      <div>
        <label>
          Start Time
          <input
            type="datetime-local"
            value={startTime}
            onChange={(e) => setStartTime(e.target.value)}
            required
          />
        </label>
      </div>
      {type === "event_duration" && (
        <div>
          <label>
            End Time
            <input
              type="datetime-local"
              value={endTime}
              onChange={(e) => setEndTime(e.target.value)}
            />
          </label>
        </div>
      )}
      {(type === "scale" || type === "environmental") && (
        <div>
          <label>
            Numeric Value
            <input
              type="number"
              step="any"
              value={numeric}
              onChange={(e) => setNumeric(e.target.value)}
              required
            />
          </label>
        </div>
      )}
      {type === "scale" && (
        <div>
          <label>
            Max
            <input type="number" value={max} onChange={(e) => setMax(e.target.value)} required />
          </label>
        </div>
      )}
      {type === "symptom" && (
        <div>
          <label>
            Severity (1-10)
            <input
              type="number"
              min="1"
              max="10"
              value={severity}
              onChange={(e) => setSeverity(e.target.value)}
              required
            />
          </label>
        </div>
      )}
      {type === "environmental" && (
        <div>
          <label>
            Unit
            <input value={unitVal} onChange={(e) => setUnitVal(e.target.value)} required />
          </label>
        </div>
      )}
      {(type === "note" ||
        type === "event_instant" ||
        type === "event_duration" ||
        type === "context_tag") && (
        <div>
          <label>
            {type === "note" ? "Text" : "Notes"}
            <textarea
              value={notesText}
              onChange={(e) => setNotesText(e.target.value)}
              required={type === "note"}
            />
          </label>
        </div>
      )}
      <button type="submit" disabled={mutation.isPending}>
        {mutation.isPending ? "Saving..." : "Save Observation"}
      </button>
      {mutation.isError && <p style={{ color: "red" }}>Error: {mutation.error.message}</p>}
      {mutation.isSuccess && <p style={{ color: "green" }}>Saved!</p>}
    </form>
  );
}
