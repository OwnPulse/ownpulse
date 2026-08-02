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

function tabId(tab: Tab): string {
  return `data-entry-tab-${tab.toLowerCase().replace(/\s+/g, "-")}`;
}

function panelId(tab: Tab): string {
  return `data-entry-panel-${tab.toLowerCase().replace(/\s+/g, "-")}`;
}

export default function DataEntry() {
  const [activeTab, setActiveTab] = useState<Tab>("Check-in");

  const handleTabKeyDown = (e: React.KeyboardEvent, index: number) => {
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    e.preventDefault();
    const delta = e.key === "ArrowRight" ? 1 : -1;
    const nextIndex = (index + delta + TABS.length) % TABS.length;
    const nextTab = TABS[nextIndex];
    setActiveTab(nextTab);
    document.getElementById(tabId(nextTab))?.focus();
  };

  return (
    <main className="op-page">
      <h1>Data Entry</h1>
      <div className="op-tab-bar" role="tablist" aria-label="Data entry type">
        {TABS.map((tab, index) => (
          <button
            type="button"
            key={tab}
            id={tabId(tab)}
            role="tab"
            aria-selected={activeTab === tab}
            aria-controls={panelId(tab)}
            tabIndex={activeTab === tab ? 0 : -1}
            className={`op-tab${activeTab === tab ? " active" : ""}`}
            onClick={() => setActiveTab(tab)}
            onKeyDown={(e) => handleTabKeyDown(e, index)}
          >
            {tab}
          </button>
        ))}
      </div>
      <div className={styles.content}>
        {TABS.map(
          (tab) =>
            activeTab === tab && (
              <div key={tab} id={panelId(tab)} role="tabpanel" aria-labelledby={tabId(tab)}>
                {tab === "Check-in" && <CheckinForm />}
                {tab === "Intervention" && <InterventionForm />}
                {tab === "Health Record" && <HealthRecordForm />}
                {tab === "Observation" && <ObservationForm />}
                {tab === "Lab Result" && <LabResultForm />}
              </div>
            ),
        )}
      </div>
    </main>
  );
}
