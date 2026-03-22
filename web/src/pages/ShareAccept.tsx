// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useEffect, useState } from "react";
import { Link, Navigate, useSearchParams } from "react-router-dom";
import { friendsApi } from "../api/friends";
import { useAuthStore } from "../store/auth";
import styles from "./ShareAccept.module.css";

export default function ShareAccept() {
  const [searchParams] = useSearchParams();
  const token = searchParams.get("token");
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
  const [status, setStatus] = useState<"loading" | "success" | "error">("loading");
  const [errorMessage, setErrorMessage] = useState("");

  useEffect(() => {
    if (!token || !isAuthenticated) return;

    let cancelled = false;
    friendsApi
      .acceptLink(token)
      .then(() => {
        if (!cancelled) setStatus("success");
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setStatus("error");
          setErrorMessage(err.message || "Failed to accept share.");
        }
      });

    return () => {
      cancelled = true;
    };
  }, [token, isAuthenticated]);

  if (!isAuthenticated) {
    const returnUrl = `/share/accept?token=${encodeURIComponent(token ?? "")}`;
    return <Navigate to={`/login?returnTo=${encodeURIComponent(returnUrl)}`} replace />;
  }

  if (!token) {
    return (
      <main className={`op-page ${styles.page}`}>
        <h1>Invalid Link</h1>
        <p>No invite token found in this link.</p>
        <Link to="/friends">Go to Friends</Link>
      </main>
    );
  }

  return (
    <main className={`op-page ${styles.page}`}>
      {status === "loading" && (
        <>
          <h1>Accepting Invite</h1>
          <p>Please wait...</p>
        </>
      )}
      {status === "success" && (
        <>
          <h1>Share Accepted</h1>
          <p>You can now view your friend&apos;s shared data.</p>
          <Link to="/friends">Go to Friends</Link>
        </>
      )}
      {status === "error" && (
        <>
          <h1>Error</h1>
          <p className="op-error-msg">{errorMessage}</p>
          <Link to="/friends">Go to Friends</Link>
        </>
      )}
    </main>
  );
}
