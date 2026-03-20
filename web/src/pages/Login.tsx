// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useState } from "react";
import { Navigate } from "react-router-dom";
import { useAuthStore } from "../store/auth";
import { useAuth } from "../hooks/useAuth";
import { login } from "../api/auth";

export default function Login() {
  const { loading } = useAuth();
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [submitting, setSubmitting] = useState(false);

  if (loading) {
    return <div>Loading...</div>;
  }

  if (isAuthenticated) {
    return <Navigate to="/" replace />;
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    setSubmitting(true);
    try {
      await login(username, password);
    } catch {
      setError("Invalid username or password.");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <main
      style={{
        maxWidth: "400px",
        margin: "4rem auto",
        padding: "2rem",
      }}
    >
      <h1>OwnPulse</h1>

      <a
        href="/api/v1/auth/google/callback"
        style={{
          display: "block",
          textAlign: "center",
          padding: "0.75rem",
          border: "1px solid #ddd",
          borderRadius: "4px",
          textDecoration: "none",
          color: "#333",
          marginBottom: "1.5rem",
        }}
      >
        Sign in with Google
      </a>

      <hr style={{ margin: "1.5rem 0" }} />

      <form onSubmit={handleSubmit}>
        <div style={{ marginBottom: "1rem" }}>
          <label>
            Username
            <input
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              required
              style={{ display: "block", width: "100%", padding: "0.5rem" }}
            />
          </label>
        </div>
        <div style={{ marginBottom: "1rem" }}>
          <label>
            Password
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
              style={{ display: "block", width: "100%", padding: "0.5rem" }}
            />
          </label>
        </div>
        {error && <p style={{ color: "red" }}>{error}</p>}
        <button
          type="submit"
          disabled={submitting}
          style={{ width: "100%", padding: "0.75rem" }}
        >
          {submitting ? "Signing in..." : "Sign In"}
        </button>
      </form>
    </main>
  );
}
