// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type CreateIntervention, interventionsApi } from "../../api/interventions";
import forms from "./forms.module.css";

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
    <form onSubmit={handleSubmit} className={forms.form}>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="intervention-substance">
          Substance
        </label>
        <input
          id="intervention-substance"
          value={substance}
          onChange={(e) => setSubstance(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="intervention-dose">
          Dose
        </label>
        <input
          id="intervention-dose"
          type="number"
          step="any"
          value={dose}
          onChange={(e) => setDose(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="intervention-unit">
          Unit
        </label>
        <input
          id="intervention-unit"
          value={unit}
          onChange={(e) => setUnit(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="intervention-route">
          Route
        </label>
        <input
          id="intervention-route"
          value={route}
          onChange={(e) => setRoute(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="intervention-time">
          Administered At
        </label>
        <input
          id="intervention-time"
          type="datetime-local"
          value={administeredAt}
          onChange={(e) => setAdministeredAt(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      <div className={forms.checkboxField}>
        <input
          type="checkbox"
          id="intervention-fasted"
          checked={fasted}
          onChange={(e) => setFasted(e.target.checked)}
        />
        <label htmlFor="intervention-fasted" className={forms.checkboxLabel}>
          Fasted
        </label>
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="intervention-notes">
          Notes
        </label>
        <textarea
          id="intervention-notes"
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
          className={forms.textarea}
        />
      </div>
      <div className={forms.actions}>
        <button type="submit" disabled={mutation.isPending} className="op-btn op-btn-primary">
          {mutation.isPending ? "Saving..." : "Save Intervention"}
        </button>
      </div>
      {mutation.isError && <p className={forms.errorMsg}>Error: {mutation.error.message}</p>}
      {mutation.isSuccess && <p className={forms.successMsg}>Saved!</p>}
    </form>
  );
}
