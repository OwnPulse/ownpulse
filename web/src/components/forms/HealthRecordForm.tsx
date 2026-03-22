// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type CreateHealthRecord, healthRecordsApi } from "../../api/health-records";
import forms from "./forms.module.css";

export default function HealthRecordForm() {
  const queryClient = useQueryClient();
  const [source, setSource] = useState("");
  const [recordType, setRecordType] = useState("");
  const [value, setValue] = useState("");
  const [unit, setUnit] = useState("");
  const [startTime, setStartTime] = useState("");

  const mutation = useMutation({
    mutationFn: (data: CreateHealthRecord) => healthRecordsApi.create(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["health-records"] });
      setSource("");
      setRecordType("");
      setValue("");
      setUnit("");
      setStartTime("");
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    mutation.mutate({
      source,
      record_type: recordType,
      value: parseFloat(value),
      unit,
      start_time: startTime,
    });
  };

  return (
    <form onSubmit={handleSubmit} className={forms.form}>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="hr-source">
          Source
        </label>
        <input
          id="hr-source"
          value={source}
          onChange={(e) => setSource(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="hr-type">
          Record Type
        </label>
        <input
          id="hr-type"
          value={recordType}
          onChange={(e) => setRecordType(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="hr-value">
          Value
        </label>
        <input
          id="hr-value"
          type="number"
          step="any"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="hr-unit">
          Unit
        </label>
        <input
          id="hr-unit"
          value={unit}
          onChange={(e) => setUnit(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      <div className={forms.field}>
        <label className={forms.label} htmlFor="hr-time">
          Start Time
        </label>
        <input
          id="hr-time"
          type="datetime-local"
          value={startTime}
          onChange={(e) => setStartTime(e.target.value)}
          required
          className={forms.input}
        />
      </div>
      <div className={forms.actions}>
        <button type="submit" disabled={mutation.isPending} className="op-btn op-btn-primary">
          {mutation.isPending ? "Saving..." : "Save Health Record"}
        </button>
      </div>
      {mutation.isError && <p className={forms.errorMsg}>Error: {mutation.error.message}</p>}
      {mutation.isSuccess && <p className={forms.successMsg}>Saved!</p>}
    </form>
  );
}
