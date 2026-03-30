// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { register } from "../api/auth";
import styles from "./Register.module.css";

const OAUTH_ERROR_MESSAGES: Record<string, string> = {
  invite_required:
    "An invite code is required to create an account. Enter your invite code below, then try signing in with Google again.",
};

export default function Register() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const inviteFromUrl = searchParams.get("invite") || "";
  const errorFromUrl = searchParams.get("error") || "";

  const [inviteCode, setInviteCode] = useState(inviteFromUrl);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [error, setError] = useState(
    errorFromUrl in OAUTH_ERROR_MESSAGES ? OAUTH_ERROR_MESSAGES[errorFromUrl] : "",
  );
  const [submitting, setSubmitting] = useState(false);

  if (!inviteFromUrl && !inviteCode && !errorFromUrl) {
    return (
      <div className="op-auth-page">
        <main className="op-auth-card">
          <h1>Sign Up</h1>
          <p className="op-empty">You need an invite code to sign up. Ask an admin for one.</p>
          <div className={styles.footer}>
            <Link to="/login" className={styles.footerLink}>
              Already have an account? Sign in
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
      await register(email, password, inviteCode);
      navigate("/");
    } catch {
      setError("Registration failed. The invite code may be invalid or expired.");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="op-auth-page">
      <main className="op-auth-card">
        <h1>Create Account</h1>

        {error && <div className={`op-error-msg ${styles.errorMsg}`}>{error}</div>}

        <a
          href={`/api/v1/auth/google/login?invite_code=${encodeURIComponent(inviteCode)}`}
          className={styles.googleBtn}
        >
          Sign up with Google
        </a>

        <div className={styles.dividerText}>or</div>

        <form onSubmit={handleSubmit}>
          <div className="op-form-field">
            <label htmlFor="register-invite" className="op-label">
              Invite Code
            </label>
            <input
              id="register-invite"
              type="text"
              value={inviteCode}
              onChange={(e) => setInviteCode(e.target.value)}
              required
              className={`op-input ${styles.monoInput}`}
            />
          </div>
          <div className="op-form-field">
            <label htmlFor="register-email" className="op-label">
              Email
            </label>
            <input
              id="register-email"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
              autoComplete="email"
              className="op-input"
            />
          </div>
          <div className="op-form-field">
            <label htmlFor="register-password" className="op-label">
              Password
            </label>
            <input
              id="register-password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
              autoComplete="new-password"
              className="op-input"
            />
          </div>
          <div className="op-form-field">
            <label htmlFor="register-confirm-password" className="op-label">
              Confirm Password
            </label>
            <input
              id="register-confirm-password"
              type="password"
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              required
              autoComplete="new-password"
              className="op-input"
            />
          </div>
          <button
            type="submit"
            disabled={submitting}
            className={`op-btn op-btn-primary ${styles.fullBtn}`}
          >
            {submitting ? "Creating account\u2026" : "Create Account"}
          </button>
        </form>

        <div className={styles.footer}>
          <Link to="/login" className={styles.footerLink}>
            Already have an account? Sign in
          </Link>
        </div>
      </main>
    </div>
  );
}
