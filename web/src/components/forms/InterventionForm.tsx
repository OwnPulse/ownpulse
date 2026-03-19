// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  interventionsApi,
  CreateIntervention,
} from "../../api/interventions";

export default function InterventionForm() {
  const queryClient = useQueryClient();
  const [substance, setSubstance] = useState("");
  const [dose, setDose] = useState("");
  const [unit, setUnit] = useState("");
  const [route, setRoute] = useState("");
  const [administeredAt, setAdministeredAt] = useState("");
  const [fasted, setFasted] = useState(false);
  const [notes, setNotes] = useState("");

  const mutation = useMutation({
    mutationFn: (data: CreateIntervention) => interventionsApi.create(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["interventions"] });
      setSubstance("");
      setDose("");
      setUnit("");
      setRoute("");
      setAdministeredAt("");
      setFasted(false);
      setNotes("");
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({
      substance,
      dose: parseFloat(dose),
      unit,
      route,
      administered_at: administeredAt,
      fasted,
      notes: notes || undefined,
    });
  };

  return (
    <form onSubmit={handleSubmit}>
      <div>
        <label>
          Substance
          <input
            value={substance}
            onChange={(e) => setSubstance(e.target.value)}
            required
          />
        </label>
      </div>
      <div>
        <label>
          Dose
          <input
            type="number"
            step="any"
            value={dose}
            onChange={(e) => setDose(e.target.value)}
            required
          />
        </label>
      </div>
      <div>
        <label>
          Unit
          <input
            value={unit}
            onChange={(e) => setUnit(e.target.value)}
            required
          />
        </label>
      </div>
      <div>
        <label>
          Route
          <input
            value={route}
            onChange={(e) => setRoute(e.target.value)}
            required
          />
        </label>
      </div>
      <div>
        <label>
          Administered At
          <input
            type="datetime-local"
            value={administeredAt}
            onChange={(e) => setAdministeredAt(e.target.value)}
            required
          />
        </label>
      </div>
      <div>
        <label>
          <input
            type="checkbox"
            checked={fasted}
            onChange={(e) => setFasted(e.target.checked)}
          />
          Fasted
        </label>
      </div>
      <div>
        <label>
          Notes
          <textarea value={notes} onChange={(e) => setNotes(e.target.value)} />
        </label>
      </div>
      <button type="submit" disabled={mutation.isPending}>
        {mutation.isPending ? "Saving..." : "Save Intervention"}
      </button>
      {mutation.isError && (
        <p style={{ color: "red" }}>Error: {mutation.error.message}</p>
      )}
      {mutation.isSuccess && <p style={{ color: "green" }}>Saved!</p>}
    </form>
  );
}
