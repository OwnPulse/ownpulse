// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import CheckinForm from "../components/forms/CheckinForm";
import HealthRecordForm from "../components/forms/HealthRecordForm";
import InterventionForm from "../components/forms/InterventionForm";
import LabResultForm from "../components/forms/LabResultForm";
import ObservationForm from "../components/forms/ObservationForm";
import styles from "./DataEntry.module.css";

const TABS = ["Check-in", "Intervention", "Health Record", "Observation", "Lab Result"] as const;

type Tab = (typeof TABS)[number];

export default function DataEntry() {
  const [activeTab, setActiveTab] = useState<Tab>("Check-in");

  return (
    <main className="op-page">
      <h1>Data Entry</h1>
      <div className="op-tab-bar">
        {TABS.map((tab) => (
          <button
            type="button"
            key={tab}
            className={`op-tab${activeTab === tab ? " active" : ""}`}
            onClick={() => setActiveTab(tab)}
          >
            {tab}
          </button>
        ))}
      </div>
      <div className={styles.content}>
        {activeTab === "Check-in" && <CheckinForm />}
        {activeTab === "Intervention" && <InterventionForm />}
        {activeTab === "Health Record" && <HealthRecordForm />}
        {activeTab === "Observation" && <ObservationForm />}
        {activeTab === "Lab Result" && <LabResultForm />}
      </div>
    </main>
  );
}
