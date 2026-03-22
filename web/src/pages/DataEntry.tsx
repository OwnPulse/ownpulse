// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import CheckinForm from "../components/forms/CheckinForm";
import HealthRecordForm from "../components/forms/HealthRecordForm";
import InterventionForm from "../components/forms/InterventionForm";
import LabResultForm from "../components/forms/LabResultForm";
import ObservationForm from "../components/forms/ObservationForm";

const TABS = ["Check-in", "Intervention", "Health Record", "Observation", "Lab Result"] as const;

type Tab = (typeof TABS)[number];

const tabBarStyle: React.CSSProperties = {
  display: "flex",
  gap: "0.5rem",
  padding: "1rem 1.5rem 0",
  borderBottom: "1px solid #ddd",
};

const tabStyle = (active: boolean): React.CSSProperties => ({
  padding: "0.5rem 1rem",
  border: "none",
  borderBottom: active ? "2px solid #333" : "2px solid transparent",
  background: "none",
  cursor: "pointer",
  fontWeight: active ? "bold" : "normal",
});

export default function DataEntry() {
  const [activeTab, setActiveTab] = useState<Tab>("Check-in");

  return (
    <main>
      <h1 style={{ padding: "0 1.5rem" }}>Data Entry</h1>
      <div style={tabBarStyle}>
        {TABS.map((tab) => (
          <button
            type="button"
            key={tab}
            style={tabStyle(activeTab === tab)}
            onClick={() => setActiveTab(tab)}
          >
            {tab}
          </button>
        ))}
      </div>
      <div style={{ padding: "1.5rem" }}>
        {activeTab === "Check-in" && <CheckinForm />}
        {activeTab === "Intervention" && <InterventionForm />}
        {activeTab === "Health Record" && <HealthRecordForm />}
        {activeTab === "Observation" && <ObservationForm />}
        {activeTab === "Lab Result" && <LabResultForm />}
      </div>
    </main>
  );
}
