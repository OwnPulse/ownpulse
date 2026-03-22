// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import { useSearchParams, useNavigate, Link } from "react-router-dom";
import { register } from "../api/auth";

export default function Register() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const inviteFromUrl = searchParams.get("invite") || "";

  const [inviteCode, setInviteCode] = useState(inviteFromUrl);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [error, setError] = useState("");
  const [submitting, setSubmitting] = useState(false);

  if (!inviteFromUrl && !inviteCode) {
    return (
      <div style={{ display: "flex", justifyContent: "center", alignItems: "center", minHeight: "100vh", background: "var(--color-bg)" }}>
        <main style={{ background: "var(--color-surface)", borderRadius: "var(--radius-md)", border: "1px solid var(--color-border)", padding: "2rem", maxWidth: "400px", width: "100%" }}>
          <h1 style={{ fontSize: "1.25rem", color: "var(--color-text)", marginBottom: "1rem" }}>Sign Up</h1>
          <p style={{ color: "var(--color-text-muted)", marginBottom: "1.5rem" }}>
            You need an invite code to sign up. Ask an admin for one.
          </p>
          <div style={{ textAlign: "center" }}>
            <Link to="/login" style={{ color: "var(--color-primary)", textDecoration: "none", fontSize: "0.875rem" }}>
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
    if (password.length < 8) {
      setError("Password must be at least 8 characters.");
      return;
    }
    if (password !== confirmPassword) {
      setError("Passwords do not match.");
      return;
    }
    setSubmitting(true);
    try {
      await register(username, password, inviteCode);
      navigate("/");
    } catch {
      setError("Registration failed. The invite code may be invalid or expired.");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div style={{ display: "flex", justifyContent: "center", alignItems: "center", minHeight: "100vh", background: "var(--color-bg)" }}>
      <main style={{ background: "var(--color-surface)", borderRadius: "var(--radius-md)", border: "1px solid var(--color-border)", padding: "2rem", maxWidth: "400px", width: "100%" }}>
        <h1 style={{ fontSize: "1.25rem", color: "var(--color-text)", marginBottom: "1.5rem" }}>Create Account</h1>

        <a
          href={`/api/v1/auth/google/login?invite_code=${encodeURIComponent(inviteCode)}`}
          style={{
            display: "flex", alignItems: "center", justifyContent: "center", gap: "0.5rem",
            padding: "0.625rem", borderRadius: "var(--radius-sm)", border: "1px solid var(--color-border)",
            background: "var(--color-surface)", color: "var(--color-text)", textDecoration: "none",
            fontSize: "0.875rem", fontFamily: "var(--font-family)", marginBottom: "1.5rem",
          }}
        >
          Sign up with Google
        </a>

        <div style={{ textAlign: "center", color: "var(--color-text-muted)", fontSize: "0.8125rem", marginBottom: "1.5rem" }}>or</div>

        <form onSubmit={handleSubmit}>
          <div style={{ marginBottom: "1rem" }}>
            <label style={{ display: "block", fontSize: "0.8125rem", color: "var(--color-text-muted)", marginBottom: "0.25rem" }}>Invite Code</label>
            <input type="text" value={inviteCode} onChange={(e) => setInviteCode(e.target.value)} required
              style={{ width: "100%", padding: "0.5rem", borderRadius: "var(--radius-sm)", border: "1px solid var(--color-border)", fontSize: "0.875rem", fontFamily: "monospace", boxSizing: "border-box" }} />
          </div>
          <div style={{ marginBottom: "1rem" }}>
            <label style={{ display: "block", fontSize: "0.8125rem", color: "var(--color-text-muted)", marginBottom: "0.25rem" }}>Username</label>
            <input type="text" value={username} onChange={(e) => setUsername(e.target.value)} required autoComplete="username"
              style={{ width: "100%", padding: "0.5rem", borderRadius: "var(--radius-sm)", border: "1px solid var(--color-border)", fontSize: "0.875rem", boxSizing: "border-box" }} />
          </div>
          <div style={{ marginBottom: "1rem" }}>
            <label style={{ display: "block", fontSize: "0.8125rem", color: "var(--color-text-muted)", marginBottom: "0.25rem" }}>Password</label>
            <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} required autoComplete="new-password"
              style={{ width: "100%", padding: "0.5rem", borderRadius: "var(--radius-sm)", border: "1px solid var(--color-border)", fontSize: "0.875rem", boxSizing: "border-box" }} />
          </div>
          <div style={{ marginBottom: "1rem" }}>
            <label style={{ display: "block", fontSize: "0.8125rem", color: "var(--color-text-muted)", marginBottom: "0.25rem" }}>Confirm Password</label>
            <input type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)} required autoComplete="new-password"
              style={{ width: "100%", padding: "0.5rem", borderRadius: "var(--radius-sm)", border: "1px solid var(--color-border)", fontSize: "0.875rem", boxSizing: "border-box" }} />
          </div>
          {error && <div style={{ color: "var(--color-error)", fontSize: "0.8125rem", marginBottom: "1rem" }}>{error}</div>}
          <button type="submit" disabled={submitting}
            style={{ width: "100%", padding: "0.625rem", borderRadius: "var(--radius-sm)", background: "var(--color-primary)", color: "#fff", border: "none", fontSize: "0.875rem", fontFamily: "var(--font-family)", cursor: "pointer" }}>
            {submitting ? "Creating account\u2026" : "Create Account"}
          </button>
        </form>

        <div style={{ textAlign: "center", marginTop: "1.5rem" }}>
          <Link to="/login" style={{ color: "var(--color-primary)", textDecoration: "none", fontSize: "0.875rem" }}>
            Already have an account? Sign in
          </Link>
        </div>
      </main>
    </div>
  );
}
