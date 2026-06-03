// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import { isTelemetryEnabled, setTelemetryEnabled } from "../../lib/telemetry";
import forms from "../forms/forms.module.css";

/**
 * Opt-in toggle for first-party, anonymous usage telemetry. Default OFF. When
 * enabled, the app reports anonymous navigation, action, and request-timing
 * events to our own backend only — never to any third party, and never any
 * health data. Disabling stops collection immediately and discards any buffered
 * events.
 */
export default function TelemetrySettings() {
  const [enabled, setEnabled] = useState<boolean>(() => isTelemetryEnabled());

  const handleToggle = (next: boolean) => {
    setTelemetryEnabled(next);
    setEnabled(next);
  };

  return (
    <section className="op-section">
      <h2>Anonymous Usage Telemetry</h2>
      <p>
        Share anonymous usage data to help improve OwnPulse. We send only page views, coarse action
        names, and request timing to our own backend — never any health data, and never to third
        parties. Off by default; you can turn it off again at any time.
      </p>
      <div className={forms.checkboxField}>
        <input
          type="checkbox"
          id="telemetry-enabled"
          aria-label="Anonymous usage telemetry"
          checked={enabled}
          onChange={(e) => handleToggle(e.target.checked)}
        />
        <label htmlFor="telemetry-enabled" className={forms.checkboxLabel}>
          {enabled ? "Telemetry enabled" : "Telemetry disabled"}
        </label>
      </div>
    </section>
  );
}
