// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type CreateIntervention, interventionsApi } from "../../api/interventions";
import { type ActiveSubstance, protocolsApi } from "../../api/protocols";
import { type SavedMedicine, savedMedicinesApi } from "../../api/savedMedicines";
import forms from "./forms.module.css";
import styles from "./InterventionForm.module.css";

function nowLocal() {
  return new Date().toISOString().slice(0, 16);
}

function chipLabel(s: ActiveSubstance): string {
  return `${s.substance} ${s.dose}${s.unit} ${s.route}`;
}

function savedMedicineLabel(m: SavedMedicine): string {
  const parts = [m.substance];
  if (m.dose != null) parts.push(String(m.dose));
  if (m.unit) parts[parts.length - 1] += m.unit;
  if (m.route) parts.push(m.route);
  return parts.join(" ");
}

export default function InterventionForm() {
  const queryClient = useQueryClient();
  const [substance, setSubstance] = useState("");
  const [dose, setDose] = useState("");
  const [unit, setUnit] = useState("");
  const [route, setRoute] = useState("");
  const [administeredAt, setAdministeredAt] = useState(nowLocal);
  const [fasted, setFasted] = useState(false);
  const [notes, setNotes] = useState("");

  const [deletingId, setDeletingId] = useState<string | null>(null);

  const activeSubstances = useQuery({
    queryKey: ["protocols", "active-substances"],
    queryFn: () => protocolsApi.activeSubstances(),
  });

  const savedMedicines = useQuery({
    queryKey: ["saved-medicines"],
    queryFn: () => savedMedicinesApi.list(),
  });

  const mutation = useMutation({
    mutationFn: (data: CreateIntervention) => interventionsApi.create(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["interventions"] });
      setSubstance("");
      setDose("");
      setUnit("");
      setRoute("");
      setAdministeredAt(nowLocal());
      setFasted(false);
      setNotes("");
    },
  });

  const saveMedicineMutation = useMutation({
    mutationFn: () =>
      savedMedicinesApi.create({
        substance,
        dose: dose ? parseFloat(dose) : undefined,
        unit: unit || undefined,
        route: route || undefined,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["saved-medicines"] });
    },
  });

  const deleteMedicineMutation = useMutation({
    mutationFn: (id: string) => savedMedicinesApi.remove(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["saved-medicines"] });
      setDeletingId(null);
    },
  });

  const handleChipClick = (s: ActiveSubstance) => {
    setSubstance(s.substance);
    setDose(String(s.dose));
    setUnit(s.unit);
    setRoute(s.route);
  };

  const handleSavedMedicineClick = (m: SavedMedicine) => {
    setSubstance(m.substance);
    if (m.dose != null) setDose(String(m.dose));
    if (m.unit) setUnit(m.unit);
    if (m.route) setRoute(m.route);
  };

  const handleDeleteMedicine = (id: string) => {
    if (deletingId === id) {
      deleteMedicineMutation.mutate(id);
    } else {
      setDeletingId(id);
    }
  };

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

  const substances = activeSubstances.data;
  const medicines = savedMedicines.data;

  return (
    <form onSubmit={handleSubmit} className={forms.form}>
      {medicines && medicines.length > 0 && (
        <div className={styles.savedMedicines} data-testid="saved-medicines-section">
          <span className={styles.quickPickLabel}>My Medicines:</span>
          <div className={styles.chipContainer}>
            {medicines.map((m) => (
              <span key={m.id} className={styles.savedChipWrapper}>
                <button
                  type="button"
                  className={styles.chip}
                  onClick={() => handleSavedMedicineClick(m)}
                >
                  {savedMedicineLabel(m)}
                </button>
                <button
                  type="button"
                  className={styles.deleteChipBtn}
                  aria-label={`Delete ${m.substance}`}
                  onClick={() => handleDeleteMedicine(m.id)}
                >
                  {deletingId === m.id ? "?" : "\u00d7"}
                </button>
              </span>
            ))}
            <button
              type="button"
              className={styles.addBtn}
              disabled={!substance.trim()}
              onClick={() => saveMedicineMutation.mutate()}
              aria-label="Save current medicine"
            >
              +
            </button>
          </div>
        </div>
      )}
      {medicines && medicines.length === 0 && substance.trim() && (
        <div className={styles.savedMedicines} data-testid="saved-medicines-section">
          <span className={styles.quickPickLabel}>My Medicines:</span>
          <div className={styles.chipContainer}>
            <button
              type="button"
              className={styles.addBtn}
              onClick={() => saveMedicineMutation.mutate()}
              aria-label="Save current medicine"
            >
              +
            </button>
          </div>
        </div>
      )}
      {substances && substances.length > 0 && (
        <div className={styles.quickPick} data-testid="quick-pick-section">
          <span className={styles.quickPickLabel}>Quick pick:</span>
          <div className={styles.chipContainer}>
            {substances.map((s) => (
              <button
                key={`${s.protocol_id}-${s.substance}`}
                type="button"
                className={styles.chip}
                onClick={() => handleChipClick(s)}
              >
                {chipLabel(s)}
              </button>
            ))}
          </div>
        </div>
      )}
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
