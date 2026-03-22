// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { type CreateHealthRecord, healthRecordsApi } from "../../api/health-records";

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
    <form onSubmit={handleSubmit}>
      <div>
        <label>
          Source
          <input value={source} onChange={(e) => setSource(e.target.value)} required />
        </label>
      </div>
      <div>
        <label>
          Record Type
          <input value={recordType} onChange={(e) => setRecordType(e.target.value)} required />
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
          Start Time
          <input
            type="datetime-local"
            value={startTime}
            onChange={(e) => setStartTime(e.target.value)}
            required
          />
        </label>
      </div>
      <button type="submit" disabled={mutation.isPending}>
        {mutation.isPending ? "Saving..." : "Save Health Record"}
      </button>
      {mutation.isError && <p style={{ color: "red" }}>Error: {mutation.error.message}</p>}
      {mutation.isSuccess && <p style={{ color: "green" }}>Saved!</p>}
    </form>
  );
}
