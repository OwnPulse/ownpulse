// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { type CreateIntervention, interventionsApi } from "../../api/interventions";
import { type ActiveSubstance, protocolsApi } from "../../api/protocols";
import { type SavedMedicine, savedMedicinesApi } from "../../api/savedMedicines";
import { localNow } from "../../utils/datetime";
import forms from "./forms.module.css";
import styles from "./InterventionForm.module.css";

function chipLabel(s: ActiveSubstance): string {
  const parts = [s.substance];
  if (s.dose != null) parts.push(s.unit != null ? `${s.dose}${s.unit}` : String(s.dose));
  if (s.route != null) parts.push(s.route);
  return parts.join(" ");
}

function savedMedicineLabel(m: SavedMedicine): string {
  const parts = [m.substance];
  if (m.dose != null) parts.push(String(m.dose));
  if (m.unit) parts[parts.length - 1] += m.unit;
  if (m.route) parts.push(m.route);
  return parts.join(" ");
}

// Free text is always allowed — these are suggestions only, matching iOS's
// unit/route pickers, not a whitelist. Never validate substance/unit/route
// input; the platform is non-judgmental by design.
const UNIT_SUGGESTIONS = ["mg", "mcg", "mL", "IU", "g", "drops", "puffs"];
const ROUTE_SUGGESTIONS = [
  "oral",
  "sublingual",
  "subq",
  "IM",
  "IV",
  "topical",
  "inhaled",
  "nasal",
  "rectal",
  "transdermal",
];

export default function InterventionForm() {
  const queryClient = useQueryClient();
  const [substance, setSubstance] = useState("");
  const [dose, setDose] = useState("");
  const [unit, setUnit] = useState("");
  const [route, setRoute] = useState("");
  const [administeredAt, setAdministeredAt] = useState(localNow);
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

  const todaysDoses = useQuery({
    queryKey: ["todays-doses"],
    queryFn: () => protocolsApi.todaysDoses(),
    staleTime: 5 * 60 * 1000,
  });

  // Today-only attribution: if the entered substance+dose matches a pending
  // scheduled dose for today, offer to log it against that protocol run
  // instead of creating a standalone intervention.
  const parsedDose = dose ? parseFloat(dose) : null;
  const matchedDose = todaysDoses.data?.find(
    (td) =>
      td.status === "pending" &&
      td.substance.trim().toLowerCase() === substance.trim().toLowerCase() &&
      parsedDose != null &&
      td.dose === parsedDose,
  );

  const [attributeToProtocol, setAttributeToProtocol] = useState(true);
  // Re-default to checked whenever the matched dose identity changes (not
  // referenced in the body — this only resets a manual uncheck from a
  // previous match).
  // biome-ignore lint/correctness/useExhaustiveDependencies: intentional reset trigger, not a value read
  useEffect(() => {
    setAttributeToProtocol(true);
  }, [matchedDose?.run_id, matchedDose?.day_number]);

  const resetForm = () => {
    setSubstance("");
    setDose("");
    setUnit("");
    setRoute("");
    setAdministeredAt(localNow());
    setFasted(false);
    setNotes("");
  };

  const mutation = useMutation({
    mutationFn: (data: CreateIntervention) => interventionsApi.create(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["interventions"] });
      resetForm();
    },
  });

  const logDoseMutation = useMutation({
    mutationFn: (data: { runId: string; protocolLineId: string; dayNumber: number }) =>
      protocolsApi.logRunDose(data.runId, {
        protocol_line_id: data.protocolLineId,
        day_number: data.dayNumber,
        administered_at: new Date(administeredAt).toISOString(),
        notes: notes || undefined,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["todays-doses"] });
      queryClient.invalidateQueries({ queryKey: ["missed-doses"] });
      queryClient.invalidateQueries({ queryKey: ["active-runs"] });
      queryClient.invalidateQueries({ queryKey: ["protocols"] });
      resetForm();
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
    if (s.dose != null) setDose(String(s.dose));
    if (s.unit != null) setUnit(s.unit);
    if (s.route != null) setRoute(s.route);
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
    if (matchedDose && attributeToProtocol) {
      logDoseMutation.mutate({
        runId: matchedDose.run_id,
        protocolLineId: matchedDose.protocol_line_id,
        dayNumber: matchedDose.day_number,
      });
      return;
    }
    mutation.mutate({
      substance,
      dose: parseFloat(dose),
      unit,
      route,
      // The datetime-local input's value has no UTC offset; the backend's
      // DateTime<Utc> requires one.
      administered_at: new Date(administeredAt).toISOString(),
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
                  className={`${styles.deleteChipBtn}${
                    deletingId === m.id ? ` ${styles.deleteChipBtnConfirming}` : ""
                  }`}
                  aria-label={`Delete ${m.substance}`}
                  onClick={() => handleDeleteMedicine(m.id)}
                >
                  {deletingId === m.id ? "Delete?" : "\u00d7"}
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
          list="intervention-unit-suggestions"
        />
        <datalist id="intervention-unit-suggestions">
          {UNIT_SUGGESTIONS.map((u) => (
            <option key={u} value={u} />
          ))}
        </datalist>
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
          list="intervention-route-suggestions"
        />
        <datalist id="intervention-route-suggestions">
          {ROUTE_SUGGESTIONS.map((r) => (
            <option key={r} value={r} />
          ))}
        </datalist>
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
      {matchedDose && (
        <div className={forms.checkboxField}>
          <input
            type="checkbox"
            id="intervention-attribute-protocol"
            checked={attributeToProtocol}
            onChange={(e) => setAttributeToProtocol(e.target.checked)}
          />
          <label htmlFor="intervention-attribute-protocol" className={forms.checkboxLabel}>
            Count toward {matchedDose.protocol_name}
          </label>
        </div>
      )}
      <div className={forms.actions}>
        <button
          type="submit"
          disabled={mutation.isPending || logDoseMutation.isPending}
          className="op-btn op-btn-primary"
        >
          {mutation.isPending || logDoseMutation.isPending ? "Saving..." : "Log Intervention"}
        </button>
      </div>
      {/* Always mounted (only the text is conditional) so assistive tech
          reliably announces the result — a role="status" node that's
          inserted fresh into the DOM each time is not guaranteed to be
          picked up by screen readers. */}
      <p
        className={
          mutation.isError || logDoseMutation.isError
            ? forms.errorMsg
            : mutation.isSuccess || logDoseMutation.isSuccess
              ? forms.successMsg
              : undefined
        }
        role="status"
        aria-live="polite"
      >
        {mutation.isError && `Error: ${mutation.error.message}`}
        {logDoseMutation.isError && `Error: ${logDoseMutation.error.message}`}
        {(mutation.isSuccess || logDoseMutation.isSuccess) && "Saved!"}
      </p>
    </form>
  );
}
