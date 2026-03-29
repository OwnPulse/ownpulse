// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { Link } from "react-router-dom";
import styles from "./Welcome.module.css";

const FEATURES = [
  {
    icon: "1",
    title: "Track health metrics",
    desc: "Connect wearables or enter data manually -- sleep, heart rate, activity, and more.",
  },
  {
    icon: "2",
    title: "Log check-ins and interventions",
    desc: "Record daily subjective scores and track supplements, medications, and substances.",
  },
  {
    icon: "3",
    title: "Upload genetic data",
    desc: "Import 23andMe or AncestryDNA files to view your genetic variants.",
  },
  {
    icon: "4",
    title: "Explore trends and correlations",
    desc: "Chart any metrics together and discover how your interventions affect your health.",
  },
];

const STEPS = [
  "Connect a wearable data source (or start with manual entry)",
  "Log your first daily check-in",
];

export default function Welcome() {
  return (
    <div className={styles.page}>
      <main className={styles.card}>
        <h1 className={styles.title}>Welcome to OwnPulse</h1>
        <p className={styles.subtitle}>Here is what you can do:</p>

        <div className={styles.features}>
          {FEATURES.map((f) => (
            <div key={f.icon} className={styles.feature}>
              <div className={styles.featureIcon}>{f.icon}</div>
              <div className={styles.featureText}>
                <div className={styles.featureTitle}>{f.title}</div>
                <div className={styles.featureDesc}>{f.desc}</div>
              </div>
            </div>
          ))}
        </div>

        <div className={styles.stepsSection}>
          <div className={styles.stepsTitle}>Get started by:</div>
          {STEPS.map((step, i) => (
            <div key={step} className={styles.step}>
              <span className={styles.stepNumber}>{i + 1}</span>
              {step}
            </div>
          ))}
        </div>

        <Link to="/" className={styles.ctaButton}>
          Go to Dashboard
        </Link>
      </main>
    </div>
  );
}
