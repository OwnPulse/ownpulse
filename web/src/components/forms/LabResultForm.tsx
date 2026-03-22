// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type CreateLabResult, labsApi } from "../../api/labs";
import forms from "./forms.module.css";

export default function LabResultForm() {
  const queryClient = useQueryClient();
  const [panelDate, setPanelDate] = useState("");
  const [labName, setLabName] = useState("");
  const [marker, setMarker] = useState("");
  const [value, setValue] = useState("");
  const [unit, setUnit] = useState("");
  const [referenceLow, setReferenceLow] = useState("");
  const [referenceHigh, setReferenceHigh] = useState("");

  const mutation = useMutation({
    mutationFn: (data: CreateLabResult) => labsApi.create(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["labs"] });
      setPanelDate("");
      setLabName("");
      setMarker("");
      setValue("");
      setUnit("");
      setReferenceLow("");
      setReferenceHigh("");
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({
      panel_date: panelDate,
      lab_name: labName,
      marker,
      value: parseFloat(value),
      unit,
      reference_low: referenceLow ? parseFloat(referenceLow) : undefined,
      reference_high: referenceHigh ? parseFloat(referenceHigh) : undefined,
    });
  };

  return (
    <form onSubmit={handleSubmit} className={forms.form}>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="lab-date">
          Panel Date
        </label>
        <input
          id="lab-date"
          type="date"
          value={panelDate}
          onChange={(e) => setPanelDate(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="lab-name">
          Lab Name
        </label>
        <input
          id="lab-name"
          value={labName}
          onChange={(e) => setLabName(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="lab-marker">
          Marker
        </label>
        <input
          id="lab-marker"
          value={marker}
          onChange={(e) => setMarker(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="lab-value">
          Value
        </label>
        <input
          id="lab-value"
          type="number"
          step="any"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="lab-unit">
          Unit
        </label>
        <input
          id="lab-unit"
          value={unit}
          onChange={(e) => setUnit(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="lab-ref-low">
          Reference Low
        </label>
        <input
          id="lab-ref-low"
          type="number"
          step="any"
          value={referenceLow}
          onChange={(e) => setReferenceLow(e.target.value)}
          className={forms.input}
        />
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="lab-ref-high">
          Reference High
        </label>
        <input
          id="lab-ref-high"
          type="number"
          step="any"
          value={referenceHigh}
          onChange={(e) => setReferenceHigh(e.target.value)}
          className={forms.input}
        />
      </div>
      <div className={forms.actions}>
        <button type="submit" disabled={mutation.isPending} className="op-btn op-btn-primary">
          {mutation.isPending ? "Saving..." : "Save Lab Result"}
        </button>
      </div>
      {mutation.isError && <p className={forms.errorMsg}>Error: {mutation.error.message}</p>}
      {mutation.isSuccess && <p className={forms.successMsg}>Saved!</p>}
    </form>
  );
}
