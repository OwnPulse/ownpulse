// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import type { Protocol, ProtocolRun, TodaysDose, UpdateRunRequest } from "../api/protocols";
import { protocolsApi } from "../api/protocols";
import { DoseStatusGrid } from "../components/protocols/DoseStatusGrid";
import { StartRunModal } from "../components/protocols/StartRunModal";
import styles from "./ProtocolView.module.css";

function runStatusBadgeClass(status: ProtocolRun["status"]): string {
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

function computeTodaysDoses(protocol: Protocol, run: ProtocolRun | null): TodaysDose[] {
  if (!run) return [];
  const todayDay = Math.floor((Date.now() - new Date(run.start_date).getTime()) / 86400000);
  if (todayDay < 0 || todayDay >= protocol.duration_days) return [];

  return protocol.lines
    .filter((line) => line.schedule_pattern[todayDay])
    .map((line) => {
      const dose = line.doses.find((d) => d.day_number === todayDay);
      return {
        protocol_id: protocol.id,
        protocol_name: protocol.name,
        protocol_line_id: line.id,
        run_id: protocol.id, // TODO: use actual run_id once ProtocolView is updated for runs
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
  const [showStartRun, setShowStartRun] = useState(false);

  const {
    data: protocol,
    isLoading,
    isError,
  } = useQuery({
    queryKey: ["protocols", id],
    queryFn: () => {
      if (!id) throw new Error("Missing protocol id");
      return protocolsApi.get(id);
    },
    enabled: !!id,
  });

  const { data: runs } = useQuery({
    queryKey: ["protocol-runs", id],
    queryFn: () => {
      if (!id) throw new Error("Missing protocol id");
      return protocolsApi.listRuns(id);
    },
    enabled: !!id,
  });

  const activeRun = runs?.find((r) => r.status === "active") ?? null;

  const logDose = useMutation({
    mutationFn: (data: { protocolLineId: string; dayNumber: number }) => {
      if (!id) throw new Error("Missing protocol id");
      return protocolsApi.logDose(id, {
        protocol_line_id: data.protocolLineId,
        day_number: data.dayNumber,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["protocols", id] });
      queryClient.invalidateQueries({ queryKey: ["protocols"] });
      queryClient.invalidateQueries({ queryKey: ["active-runs"] });
    },
  });

  const skipDose = useMutation({
    mutationFn: (data: { protocolLineId: string; dayNumber: number }) => {
      if (!id) throw new Error("Missing protocol id");
      return protocolsApi.skipDose(id, {
        protocol_line_id: data.protocolLineId,
        day_number: data.dayNumber,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["protocols", id] });
      queryClient.invalidateQueries({ queryKey: ["protocols"] });
      queryClient.invalidateQueries({ queryKey: ["active-runs"] });
    },
  });

  const shareMutation = useMutation({
    mutationFn: () => {
      if (!id) throw new Error("Missing protocol id");
      return protocolsApi.share(id);
    },
    onSuccess: (res) => {
      const link = `${window.location.origin}/protocols/shared/${res.share_token}`;
      setShareLink(link);
    },
  });

  const updateRunMutation = useMutation({
    mutationFn: ({ runId, data }: { runId: string; data: UpdateRunRequest }) =>
      protocolsApi.updateRun(runId, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["protocol-runs", id] });
      queryClient.invalidateQueries({ queryKey: ["protocols", id] });
      queryClient.invalidateQueries({ queryKey: ["active-runs"] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: () => {
      if (!id) throw new Error("Missing protocol id");
      return protocolsApi.delete(id);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["protocols"] });
      navigate("/protocols");
    },
  });

  const handleExport = async () => {
    if (!id || !protocol) return;
    const data = await protocolsApi.exportProtocol(id);
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${protocol.name.replace(/[^a-z0-9]/gi, "-").toLowerCase()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  if (!id) return <main className="op-page">Not found</main>;
  if (isLoading) return <main className="op-page">Loading...</main>;
  if (isError || !protocol) return <main className="op-page">Error loading protocol.</main>;

  const progress = computeProgress(protocol);
  const todaysDoses = computeTodaysDoses(protocol, activeRun);
  const pct = progress.total > 0 ? Math.round((progress.completed / progress.total) * 100) : 0;

  return (
    <main className={`op-page ${styles.page}`}>
      {showStartRun && (
        <StartRunModal
          protocolId={protocol.id}
          protocolName={protocol.name}
          onClose={() => setShowStartRun(false)}
        />
      )}

      {/* Header */}
      <div className={styles.header}>
        <h1>{protocol.name}</h1>
      </div>

      <div className={styles.meta}>
        {protocol.duration_days} days
        {activeRun ? ` \u00b7 Run started ${activeRun.start_date}` : ""}
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

      {/* Runs section */}
      <section className={styles.runsSection}>
        <div className={styles.runsSectionHeader}>
          <h2>Runs</h2>
          <button
            type="button"
            className="op-btn op-btn-primary op-btn-sm"
            onClick={() => setShowStartRun(true)}
          >
            Start New Run
          </button>
        </div>
        {runs && runs.length > 0 ? (
          <div className={styles.runsList}>
            {runs.map((run) => (
              <div key={run.id} className={`op-card ${styles.runCard}`}>
                <div className={styles.runCardHeader}>
                  <span className={`${styles.badge} ${runStatusBadgeClass(run.status)}`}>
                    {run.status}
                  </span>
                  <span className={styles.runDate}>Started {run.start_date}</span>
                </div>
                <div className={styles.runActions}>
                  {run.status === "active" && (
                    <>
                      <button
                        type="button"
                        className="op-btn op-btn-ghost op-btn-sm"
                        onClick={() =>
                          updateRunMutation.mutate({
                            runId: run.id,
                            data: { status: "paused" },
                          })
                        }
                        disabled={updateRunMutation.isPending}
                      >
                        Pause
                      </button>
                      <button
                        type="button"
                        className="op-btn op-btn-ghost op-btn-sm"
                        onClick={() =>
                          updateRunMutation.mutate({
                            runId: run.id,
                            data: { status: "completed" },
                          })
                        }
                        disabled={updateRunMutation.isPending}
                      >
                        Complete
                      </button>
                    </>
                  )}
                  {run.status === "paused" && (
                    <button
                      type="button"
                      className="op-btn op-btn-ghost op-btn-sm"
                      onClick={() =>
                        updateRunMutation.mutate({
                          runId: run.id,
                          data: { status: "active" },
                        })
                      }
                      disabled={updateRunMutation.isPending}
                    >
                      Resume
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <p className={styles.emptyDoses}>No runs yet. Start your first run.</p>
        )}
      </section>

      {/* Dose status grid */}
      <section className={styles.gridSection}>
        <h2>Schedule</h2>
        <DoseStatusGrid
          lines={protocol.lines}
          startDate={activeRun?.start_date ?? protocol.start_date ?? protocol.created_at}
          durationDays={protocol.duration_days}
        />
      </section>

      {/* Today's doses (only if active run) */}
      {activeRun && (
        <section className={styles.dosesSection}>
          <h2>Today&rsquo;s Doses</h2>
          {todaysDoses.length === 0 && (
            <p className={styles.emptyDoses}>No doses scheduled for today.</p>
          )}
          {todaysDoses.map((td) => (
            <div key={td.protocol_line_id} className={`op-card ${styles.doseItem}`}>
              <div className={styles.doseInfo}>
                <span className={styles.doseSubstance}>
                  {td.substance} {td.dose}
                  {td.unit}
                </span>
                <span className={styles.doseMeta}>
                  {td.route}
                  {td.time_of_day ? ` \u00b7 ${td.time_of_day}` : ""}
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
      )}

      {/* Actions */}
      <div className={styles.actions}>
        <button type="button" className="op-btn op-btn-ghost" onClick={handleExport}>
          Export
        </button>
        <button
          type="button"
          className="op-btn op-btn-ghost"
          onClick={() => shareMutation.mutate()}
          disabled={shareMutation.isPending}
        >
          Share
        </button>
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
