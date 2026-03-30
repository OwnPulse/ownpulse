// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import type { Protocol, TodaysDose } from "../api/protocols";
import { protocolsApi } from "../api/protocols";
import { DoseStatusGrid } from "../components/protocols/DoseStatusGrid";
import styles from "./ProtocolView.module.css";

function badgeClass(status: Protocol["status"]): string {
  if (status === "active") return styles.badgeActive;
  if (status === "paused") return styles.badgePaused;
  return styles.badgeCompleted;
}

function computeProgress(protocol: Protocol): { completed: number; total: number } {
  let completed = 0;
  let total = 0;
  for (const line of protocol.lines) {
    for (let d = 0; d < protocol.duration_days; d++) {
      if (line.schedule_pattern[d]) {
        total++;
        const dose = line.doses.find((dd) => dd.day_number === d);
        if (dose && dose.status === "completed") completed++;
      }
    }
  }
  return { completed, total };
}

function computeTodaysDoses(protocol: Protocol): TodaysDose[] {
  const todayDay = Math.floor(
    (Date.now() - new Date(protocol.start_date).getTime()) / 86400000,
  );
  if (todayDay < 0 || todayDay >= protocol.duration_days) return [];

  return protocol.lines
    .filter((line) => line.schedule_pattern[todayDay])
    .map((line) => {
      const dose = line.doses.find((d) => d.day_number === todayDay);
      return {
        protocol_id: protocol.id,
        protocol_name: protocol.name,
        protocol_line_id: line.id,
        substance: line.substance,
        dose: line.dose,
        unit: line.unit,
        route: line.route,
        time_of_day: line.time_of_day,
        day_number: todayDay,
        status: dose?.status ?? "pending",
        dose_id: dose?.id ?? null,
      };
    });
}

export default function ProtocolView() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [shareLink, setShareLink] = useState<string | null>(null);

  const { data: protocol, isLoading, isError } = useQuery({
    queryKey: ["protocols", id],
    queryFn: () => protocolsApi.get(id!),
    enabled: !!id,
  });

  const logDose = useMutation({
    mutationFn: (data: { protocolLineId: string; dayNumber: number }) =>
      protocolsApi.logDose(id!, {
        protocol_line_id: data.protocolLineId,
        day_number: data.dayNumber,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["protocols", id] });
      queryClient.invalidateQueries({ queryKey: ["protocols"] });
    },
  });

  const skipDose = useMutation({
    mutationFn: (data: { protocolLineId: string; dayNumber: number }) =>
      protocolsApi.skipDose(id!, {
        protocol_line_id: data.protocolLineId,
        day_number: data.dayNumber,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["protocols", id] });
      queryClient.invalidateQueries({ queryKey: ["protocols"] });
    },
  });

  const shareMutation = useMutation({
    mutationFn: () => protocolsApi.share(id!),
    onSuccess: (res) => {
      const link = `${window.location.origin}/protocols/shared/${res.share_token}`;
      setShareLink(link);
    },
  });

  const updateMutation = useMutation({
    mutationFn: (data: Partial<Pick<Protocol, "status">>) =>
      protocolsApi.update(id!, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["protocols", id] });
      queryClient.invalidateQueries({ queryKey: ["protocols"] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: () => protocolsApi.delete(id!),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["protocols"] });
      navigate("/protocols");
    },
  });

  if (isLoading) return <main className="op-page">Loading...</main>;
  if (isError || !protocol) return <main className="op-page">Error loading protocol.</main>;

  const progress = computeProgress(protocol);
  const todaysDoses = computeTodaysDoses(protocol);
  const pct = progress.total > 0 ? Math.round((progress.completed / progress.total) * 100) : 0;

  return (
    <main className={`op-page ${styles.page}`}>
      {/* Header */}
      <div className={styles.header}>
        <h1>{protocol.name}</h1>
        <span className={`${styles.badge} ${badgeClass(protocol.status)}`}>
          {protocol.status}
        </span>
      </div>

      <div className={styles.meta}>
        Started {protocol.start_date} &middot; {protocol.duration_days} days
      </div>

      {/* Progress bar */}
      <div className={styles.progressSection}>
        <span className={styles.progressLabel}>
          {progress.completed}/{progress.total} doses completed ({pct}%)
        </span>
        <div className={styles.progressBar}>
          <div className={styles.progressFill} style={{ width: `${pct}%` }} />
        </div>
      </div>

      {/* Dose status grid */}
      <section className={styles.gridSection}>
        <h2>Schedule</h2>
        <DoseStatusGrid
          lines={protocol.lines}
          startDate={protocol.start_date}
          durationDays={protocol.duration_days}
        />
      </section>

      {/* Today's doses */}
      <section className={styles.dosesSection}>
        <h2>Today&rsquo;s Doses</h2>
        {todaysDoses.length === 0 && (
          <p className={styles.emptyDoses}>No doses scheduled for today.</p>
        )}
        {todaysDoses.map((td) => (
          <div key={td.protocol_line_id} className={`op-card ${styles.doseItem}`}>
            <div className={styles.doseInfo}>
              <span className={styles.doseSubstance}>
                {td.substance} {td.dose}{td.unit}
              </span>
              <span className={styles.doseMeta}>
                {td.route}{td.time_of_day ? ` \u00b7 ${td.time_of_day}` : ""}
              </span>
            </div>
            {td.status === "pending" ? (
              <div className={styles.doseActions}>
                <button
                  type="button"
                  className="op-btn op-btn-primary op-btn-sm"
                  onClick={() =>
                    logDose.mutate({
                      protocolLineId: td.protocol_line_id,
                      dayNumber: td.day_number,
                    })
                  }
                  disabled={logDose.isPending}
                >
                  Log
                </button>
                <button
                  type="button"
                  className="op-btn op-btn-ghost op-btn-sm"
                  onClick={() =>
                    skipDose.mutate({
                      protocolLineId: td.protocol_line_id,
                      dayNumber: td.day_number,
                    })
                  }
                  disabled={skipDose.isPending}
                >
                  Skip
                </button>
              </div>
            ) : (
              <span
                className={`${styles.doseStatus} ${td.status === "completed" ? styles.statusCompleted : styles.statusSkipped}`}
              >
                {td.status}
              </span>
            )}
          </div>
        ))}
      </section>

      {/* Actions */}
      <div className={styles.actions}>
        <button
          type="button"
          className="op-btn op-btn-ghost"
          onClick={() => shareMutation.mutate()}
          disabled={shareMutation.isPending}
        >
          Share
        </button>
        {protocol.status === "active" ? (
          <button
            type="button"
            className="op-btn op-btn-ghost"
            onClick={() => updateMutation.mutate({ status: "paused" })}
            disabled={updateMutation.isPending}
          >
            Pause
          </button>
        ) : protocol.status === "paused" ? (
          <button
            type="button"
            className="op-btn op-btn-ghost"
            onClick={() => updateMutation.mutate({ status: "active" })}
            disabled={updateMutation.isPending}
          >
            Resume
          </button>
        ) : null}
        <button
          type="button"
          className="op-btn op-btn-danger"
          onClick={() => deleteMutation.mutate()}
          disabled={deleteMutation.isPending}
        >
          Delete
        </button>
      </div>

      {shareLink && (
        <div className={styles.shareLink}>
          <div className={styles.shareLinkRow}>
            <input type="text" readOnly value={shareLink} className={styles.shareLinkInput} />
            <button
              type="button"
              className="op-btn op-btn-ghost op-btn-sm"
              onClick={() => navigator.clipboard.writeText(shareLink)}
            >
              Copy
            </button>
          </div>
        </div>
      )}

      {/* Description */}
      {protocol.description && (
        <section>
          <h2>Description</h2>
          <p className={styles.description}>{protocol.description}</p>
        </section>
      )}
    </main>
  );
}
