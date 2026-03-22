// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type CreateLabResult, labsApi } from "../../api/labs";

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
    <form onSubmit={handleSubmit}>
      <div>
        <label>
          Panel Date
          <input
            type="date"
            value={panelDate}
            onChange={(e) => setPanelDate(e.target.value)}
            required
          />
        </label>
      </div>
      <div>
        <label>
          Lab Name
          <input value={labName} onChange={(e) => setLabName(e.target.value)} required />
        </label>
      </div>
      <div>
        <label>
          Marker
          <input value={marker} onChange={(e) => setMarker(e.target.value)} required />
        </label>
      </div>
      <div>
        <label>
          Value
          <input
            type="number"
            step="any"
            value={value}
            onChange={(e) => setValue(e.target.value)}
            required
          />
        </label>
      </div>
      <div>
        <label>
          Unit
          <input value={unit} onChange={(e) => setUnit(e.target.value)} required />
        </label>
      </div>
      <div>
        <label>
          Reference Low
          <input
            type="number"
            step="any"
            value={referenceLow}
            onChange={(e) => setReferenceLow(e.target.value)}
          />
        </label>
      </div>
      <div>
        <label>
          Reference High
          <input
            type="number"
            step="any"
            value={referenceHigh}
            onChange={(e) => setReferenceHigh(e.target.value)}
          />
        </label>
      </div>
      <button type="submit" disabled={mutation.isPending}>
        {mutation.isPending ? "Saving..." : "Save Lab Result"}
      </button>
      {mutation.isError && <p style={{ color: "red" }}>Error: {mutation.error.message}</p>}
      {mutation.isSuccess && <p style={{ color: "green" }}>Saved!</p>}
    </form>
  );
}
