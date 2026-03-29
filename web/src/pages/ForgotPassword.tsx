// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import { Link } from "react-router-dom";
import { forgotPassword } from "../api/auth";
import styles from "./ForgotPassword.module.css";

export default function ForgotPassword() {
  const [email, setEmail] = useState("");
  const [error, setError] = useState("");
  const [submitted, setSubmitted] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    setSubmitting(true);
    try {
      await forgotPassword(email);
      setSubmitted(true);
    } catch {
      setError("Something went wrong. Please try again.");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className={styles.page}>
      <main className={styles.card}>
        <h1 className={styles.heading}>Reset your password</h1>
        <p className={styles.description}>
          Enter your email address and we'll send you a link to reset your password.
        </p>

        {submitted ? (
          <div className={styles.success}>
            If an account exists with that email, we've sent a password reset link. Check your
            inbox.
          </div>
        ) : (
          <form onSubmit={handleSubmit}>
            <div className={styles.field}>
              <label className={styles.label} htmlFor="forgot-email">
                Email
              </label>
              <input
                id="forgot-email"
                className={styles.input}
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                required
                autoComplete="email"
              />
            </div>
            {error && <div className={styles.error}>{error}</div>}
            <button type="submit" disabled={submitting} className={styles.submit}>
              {submitting ? "Sending\u2026" : "Send Reset Link"}
            </button>
          </form>
        )}

        <div className={styles.footer}>
          <Link to="/login" className={styles.footerLink}>
            Back to login
          </Link>
        </div>
      </main>
    </div>
  );
}
