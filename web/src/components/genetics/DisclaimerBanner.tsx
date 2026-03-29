// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import styles from "./DisclaimerBanner.module.css";

export function DisclaimerBanner({ text }: { text?: string }) {
  return (
    <div className={styles.banner} role="alert">
      <span className={styles.icon} aria-hidden="true">
        !!
      </span>
      <p className={styles.text}>
        {text ??
          "This information is for educational purposes only and should not be used for medical decisions. Consult a healthcare provider or genetic counselor for clinical interpretation."}
      </p>
    </div>
  );
}
