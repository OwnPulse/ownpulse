// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { resetPassword } from "../api/auth";
import styles from "./ResetPassword.module.css";

export default function ResetPassword() {
  const [searchParams] = useSearchParams();
  const token = searchParams.get("token");

  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [error, setError] = useState("");
  const [success, setSuccess] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  if (!token) {
    return (
      <div className={styles.page}>
        <main className={styles.card}>
          <h1 className={styles.heading}>Invalid reset link</h1>
          <p className={styles.description}>
            This password reset link is missing a token. Please request a new one.
          </p>
          <div className={styles.footer}>
            <Link to="/forgot-password" className={styles.footerLink}>
              Request a new reset link
            </Link>
          </div>
        </main>
      </div>
    );
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");

    if (password.length < 10) {
      setError("Password must be at least 10 characters.");
      return;
    }
    if (password !== confirmPassword) {
      setError("Passwords do not match.");
      return;
    }

    setSubmitting(true);
    try {
      await resetPassword(token, password);
      setSuccess(true);
    } catch (err) {
      if (err instanceof Error && "status" in err && (err as { status: number }).status === 400) {
        setError("Reset link has expired or already been used.");
      } else {
        setError("Something went wrong. Please try again.");
      }
    } finally {
      setSubmitting(false);
    }
  };

  if (success) {
    return (
      <div className={styles.page}>
        <main className={styles.card}>
          <h1 className={styles.heading}>Password reset successfully</h1>
          <p className={styles.description}>
            Your password has been updated. You can now sign in with your new password.
          </p>
          <div className={styles.footer}>
            <Link to="/login" className={styles.footerLink}>
              Back to login
            </Link>
          </div>
        </main>
      </div>
    );
  }

  return (
    <div className={styles.page}>
      <main className={styles.card}>
        <h1 className={styles.heading}>Set a new password</h1>
        <p className={styles.description}>Enter your new password below.</p>

        <form onSubmit={handleSubmit}>
          <div className={styles.field}>
            <label className={styles.label} htmlFor="reset-password">
              New Password
            </label>
            <input
              id="reset-password"
              className={styles.input}
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
              autoComplete="new-password"
            />
          </div>
          <div className={styles.field}>
            <label className={styles.label} htmlFor="reset-confirm-password">
              Confirm Password
            </label>
            <input
              id="reset-confirm-password"
              className={styles.input}
              type="password"
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              required
              autoComplete="new-password"
            />
          </div>
          {error && (
            <div className={styles.error}>
              {error}{" "}
              {error.includes("expired") && (
                <Link to="/forgot-password" className={styles.footerLink}>
                  Request a new one
                </Link>
              )}
            </div>
          )}
          <button type="submit" disabled={submitting} className={styles.submit}>
            {submitting ? "Resetting\u2026" : "Reset Password"}
          </button>
        </form>

        <div className={styles.footer}>
          <Link to="/login" className={styles.footerLink}>
            Back to login
          </Link>
        </div>
      </main>
    </div>
  );
}
