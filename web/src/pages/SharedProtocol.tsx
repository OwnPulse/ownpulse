// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "react-router-dom";
import { protocolsApi } from "../api/protocols";
import { useAuthStore } from "../store/auth";
import styles from "./SharedProtocol.module.css";

export default function SharedProtocol() {
  const { token } = useParams<{ token: string }>();
  const navigate = useNavigate();
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);

  const { data: protocol, isLoading, isError } = useQuery({
    queryKey: ["shared-protocol", token],
    queryFn: () => protocolsApi.getShared(token!),
    enabled: !!token,
  });

  const importMutation = useMutation({
    mutationFn: () => protocolsApi.importProtocol(token!),
    onSuccess: (newProtocol) => {
      navigate(`/protocols/${newProtocol.id}`);
    },
  });

  if (!token) {
    return (
      <main className={`op-page ${styles.page}`}>
        <h1>Invalid Link</h1>
        <p>No share token found.</p>
      </main>
    );
  }

  if (isLoading) return <main className="op-page">Loading...</main>;

  if (isError || !protocol) {
    return (
      <main className={`op-page ${styles.page}`}>
        <h1>Protocol Not Found</h1>
        <p className="op-error-msg">This share link is invalid or has expired.</p>
      </main>
    );
  }

  return (
    <main className={`op-page ${styles.page}`}>
      <div className={styles.header}>
        <h1>{protocol.name}</h1>
        {protocol.description && (
          <p className={styles.description}>{protocol.description}</p>
        )}
      </div>

      <section className={styles.linesSection}>
        <h2>Protocol Lines</h2>
        {protocol.lines.map((line) => (
          <div key={line.id} className={`op-card ${styles.lineItem}`}>
            <div className={styles.lineSubstance}>
              {line.substance} {line.dose}{line.unit}
            </div>
            <div className={styles.lineMeta}>
              {line.route}{line.time_of_day ? ` \u00b7 ${line.time_of_day}` : ""}
            </div>
            <div className={styles.schedulePreview}>
              {line.schedule_pattern.slice(0, Math.min(line.schedule_pattern.length, 28)).map((on, i) => (
                <div
                  key={i}
                  className={`${styles.scheduleDay} ${on ? styles.scheduleDayOn : ""}`}
                  title={`Day ${i + 1}: ${on ? "on" : "off"}`}
                />
              ))}
            </div>
          </div>
        ))}
      </section>

      <div className={styles.actions}>
        {isAuthenticated ? (
          <button
            type="button"
            className="op-btn op-btn-primary"
            onClick={() => importMutation.mutate()}
            disabled={importMutation.isPending}
          >
            {importMutation.isPending ? "Copying..." : "Copy to My Protocols"}
          </button>
        ) : (
          <p className={styles.loginMsg}>
            <Link to={`/login?returnTo=${encodeURIComponent(`/protocols/shared/${token}`)}`}>
              Log in
            </Link>{" "}
            to copy this protocol to your account.
          </p>
        )}
        {importMutation.isError && (
          <p className="op-error-msg">Failed to copy protocol. Please try again.</p>
        )}
      </div>
    </main>
  );
}
