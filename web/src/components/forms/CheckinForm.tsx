// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type CheckinInput, checkinsApi } from "../../api/checkins";
import forms from "./forms.module.css";

const DIMENSION_COLORS: Record<string, string> = {
  Energy: "#c49a3c",
  Mood: "#c2654a",
  Focus: "#3d8b8b",
  Recovery: "#5a8a5a",
  Libido: "#7b61c2",
};

function sliderBackground(value: string, color: string): string {
  const pct = ((parseInt(value, 10) - 1) / 9) * 100;
  return `linear-gradient(to right, ${color} 0%, ${color} ${pct}%, var(--color-border) ${pct}%)`;
}

function todayDate() {
  return new Date().toISOString().slice(0, 10);
}

export default function CheckinForm() {
  const queryClient = useQueryClient();
  const [date, setDate] = useState(todayDate);
  const [energy, setEnergy] = useState("5");
  const [mood, setMood] = useState("5");
  const [focus, setFocus] = useState("5");
  const [recovery, setRecovery] = useState("5");
  const [libido, setLibido] = useState("5");
  const [notes, setNotes] = useState("");

  const mutation = useMutation({
    mutationFn: (data: CheckinInput) => checkinsApi.create(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["checkins"] });
      setDate(todayDate());
      setEnergy("5");
      setMood("5");
      setFocus("5");
      setRecovery("5");
      setLibido("5");
      setNotes("");
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({
      date,
      energy: parseInt(energy, 10),
      mood: parseInt(mood, 10),
      focus: parseInt(focus, 10),
      recovery: parseInt(recovery, 10),
      libido: parseInt(libido, 10),
      notes: notes || undefined,
    });
  };

  return (
    <form onSubmit={handleSubmit} className={forms.form}>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="checkin-date">
          Date
        </label>
        <input
          id="checkin-date"
          type="date"
          value={date}
          onChange={(e) => setDate(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      {[
        { label: "Energy", value: energy, setter: setEnergy },
        { label: "Mood", value: mood, setter: setMood },
        { label: "Focus", value: focus, setter: setFocus },
        { label: "Recovery", value: recovery, setter: setRecovery },
        { label: "Libido", value: libido, setter: setLibido },
      ].map(({ label, value, setter }) => {
        const color = DIMENSION_COLORS[label];
        return (
          <div key={label} className={forms.sliderField}>
            <div className={forms.sliderLabel}>
              <span className={forms.sliderLabelText}>{label}</span>
              <span className={forms.sliderValue} style={{ color }}>
                {value}/10
              </span>
            </div>
            <input
              type="range"
              min="1"
              max="10"
              value={value}
              onChange={(e) => setter(e.target.value)}
              className="op-slider"
              style={
                {
                  background: sliderBackground(value, color),
                  "--slider-color": color,
                } as React.CSSProperties
              }
              aria-label={label}
            />
          </div>
        );
      })}
      <div className={forms.field}>
        <label className={forms.label} htmlFor="checkin-notes">
          Notes
        </label>
        <textarea
          id="checkin-notes"
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
          className={forms.textarea}
        />
      </div>
      <div className={forms.actions}>
        <button type="submit" disabled={mutation.isPending} className="op-btn op-btn-primary">
          {mutation.isPending ? "Saving..." : "Save Check-in"}
        </button>
      </div>
      {mutation.isError && <p className={forms.errorMsg}>Error: {mutation.error.message}</p>}
      {mutation.isSuccess && <p className={forms.successMsg}>Saved!</p>}
    </form>
  );
}
