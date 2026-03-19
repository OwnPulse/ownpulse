// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { checkinsApi, UpsertCheckin } from "../../api/checkins";

export default function CheckinForm() {
  const queryClient = useQueryClient();
  const [date, setDate] = useState("");
  const [energy, setEnergy] = useState("5");
  const [mood, setMood] = useState("5");
  const [focus, setFocus] = useState("5");
  const [recovery, setRecovery] = useState("5");
  const [libido, setLibido] = useState("5");
  const [notes, setNotes] = useState("");

  const mutation = useMutation({
    mutationFn: (data: UpsertCheckin) => checkinsApi.upsert(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["checkins"] });
      setDate("");
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
    <form onSubmit={handleSubmit}>
      <div>
        <label>
          Date
          <input
            type="date"
            value={date}
            onChange={(e) => setDate(e.target.value)}
            required
          />
        </label>
      </div>
      {[
        { label: "Energy", value: energy, setter: setEnergy },
        { label: "Mood", value: mood, setter: setMood },
        { label: "Focus", value: focus, setter: setFocus },
        { label: "Recovery", value: recovery, setter: setRecovery },
        { label: "Libido", value: libido, setter: setLibido },
      ].map(({ label, value, setter }) => (
        <div key={label}>
          <label>
            {label} ({value}/10)
            <input
              type="range"
              min="1"
              max="10"
              value={value}
              onChange={(e) => setter(e.target.value)}
            />
          </label>
        </div>
      ))}
      <div>
        <label>
          Notes
          <textarea value={notes} onChange={(e) => setNotes(e.target.value)} />
        </label>
      </div>
      <button type="submit" disabled={mutation.isPending}>
        {mutation.isPending ? "Saving..." : "Save Check-in"}
      </button>
      {mutation.isError && (
        <p style={{ color: "red" }}>Error: {mutation.error.message}</p>
      )}
      {mutation.isSuccess && <p style={{ color: "green" }}>Saved!</p>}
    </form>
  );
}
