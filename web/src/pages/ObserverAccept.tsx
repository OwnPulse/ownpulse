// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useEffect, useState } from "react";
import { Link, Navigate, useSearchParams } from "react-router-dom";
import { observerPollsApi } from "../api/observer-polls";
import { useAuthStore } from "../store/auth";
import styles from "./ShareAccept.module.css";

export default function ObserverAccept() {
  const [searchParams] = useSearchParams();
  const token = searchParams.get("token");
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
  const [status, setStatus] = useState<
    "loading" | "accepted" | "acknowledged" | "error"
  >("loading");
  const [errorMessage, setErrorMessage] = useState("");

  useEffect(() => {
    if (!token || !isAuthenticated) return;

    let cancelled = false;
    observerPollsApi
      .accept(token)
      .then((data) => {
        if (!cancelled) {
          setStatus(data.status);
        }
      })
      .catch((err: Error) => {
        if (!cancelled) {
          setStatus("error");
          setErrorMessage(err.message || "Failed to accept invite.");
        }
      });

    return () => {
      cancelled = true;
    };
  }, [token, isAuthenticated]);

  if (!isAuthenticated) {
    const returnUrl = `/observe/accept?token=${encodeURIComponent(token ?? "")}`;
    return (
      <Navigate
        to={`/login?returnTo=${encodeURIComponent(returnUrl)}`}
        replace
      />
    );
  }

  if (!token) {
    return (
      <main className={`op-page ${styles.page}`}>
        <h1>Invalid Link</h1>
        <p>No invite token found in this link.</p>
        <Link to="/observer-polls">Go to Observer Polls</Link>
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
      {status === "accepted" && (
        <>
          <h1>Observer Invite Accepted</h1>
          <p>
            You&apos;ve been added as an observer! Go to Observer Polls
            to start rating.
          </p>
          <Link to="/observer-polls">Go to Observer Polls</Link>
        </>
      )}
      {status === "acknowledged" && (
        <>
          <h1>Invite No Longer Valid</h1>
          <p>
            This invite link is no longer valid. Ask the poll owner for
            a new link.
          </p>
          <Link to="/observer-polls">Go to Observer Polls</Link>
        </>
      )}
      {status === "error" && (
        <>
          <h1>Error</h1>
          <p className="op-error-msg">{errorMessage}</p>
          <Link to="/observer-polls">Go to Observer Polls</Link>
        </>
      )}
    </main>
  );
}
